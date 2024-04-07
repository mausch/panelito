use bpaf::{Parser, construct};
use serde::{Deserialize, Serialize};
use std::{fmt::{Debug, Display}, fs::File, io::Write};
use anyhow::{Context, Result, Ok, bail, anyhow};
use framebuffer::Framebuffer;
use rumqttc::{MqttOptions, QoS, Client};
use ddc_hi::{Ddc, DdcHost, Display as DdcDisplay};

struct Percentage(u8);

impl Percentage {
    pub fn new(v: u8) -> Result<Self> {
        if v <= 100 {
            Ok(Percentage(v))
        } else {
            Err(anyhow!("Invalid percentage value {v}"))
        }
    }
}

impl Display for Percentage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let v = self.0;
        f.write_str(format!("{v}%").as_str())
    }
}

impl<'de> Deserialize<'de> for Percentage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
        let v = u8::deserialize(deserializer)?;

        match Percentage::new(v) {
            Result::Ok(v) => Result::Ok(v),
            Err(_) => Err(serde::de::Error::custom(format!("Invalid percentage value {v}"))),
        }
    }
}

#[repr(u8)]
enum DdcCommand {
    Brightness = 0x10,
    Power = 0xD6,
}

#[repr(u16)]
#[derive(Clone)]
enum DdcPower {
    On = 1,
    Off = 5,
}

fn set_ddc_power(power: DdcPower, mut displays: Vec<DdcDisplay>) -> Result<Vec<DdcDisplay>> {
    for display in displays.iter_mut() {
        let model = display.info.model_name.clone().unwrap_or(String::from("UNKNOWN"));
        display.handle.set_vcp_feature(DdcCommand::Power as u8, power.clone() as u16)
        .with_context(|| format!("Could not set DDC power for {model}"))?;
        display.handle.sleep();
    }
    Ok(displays)
}

fn set_brightness<'a>(percentage: &Percentage, displays: Vec<DdcDisplay>) -> Result<Vec<DdcDisplay>> {
    let displays1 = set_ddc_brightness(&percentage, displays)?;
    let _ = set_backlight_brightness(&percentage);
    Ok(displays1)
}

fn set_ddc_brightness<'a>(percentage: &Percentage, mut displays: Vec<DdcDisplay>) -> Result<Vec<DdcDisplay>> {
    for display in displays.iter_mut() {
        let model = display.info.model_name.clone().unwrap_or(String::from("UNKNOWN"));
        display.handle.set_vcp_feature(DdcCommand::Brightness as u8, percentage.0.into())
            .with_context(|| format!("Could not set DDC brightness for {model}"))?;
        display.handle.sleep();
    }
    Ok(displays)
}

fn set_backlight_brightness(percentage: &Percentage) -> Result<()> {
    // TODO iterate on /sys/class/backlight/*
    log::info!("Setting brightness to {percentage}");
    let max_brightness_raw = std::fs::read_to_string("/sys/class/backlight/intel_backlight/max_brightness")
        .with_context(|| "Could not read max brightness")?;

    let max_brightness: u32 = max_brightness_raw.trim().parse::<u32>()
        .with_context(|| format!("Could not parse max brightness from '{max_brightness_raw}'"))?;

    let mut brightness_file = File::create("/sys/class/backlight/intel_backlight/brightness")
        .with_context(|| "Could not set brightness")?;

    let brightness = (percentage.0 as f64) / 100.0 * (max_brightness as f64);

    brightness_file.write_all((brightness as u32).to_string().as_bytes())?;

    Ok(())
}

#[derive(Serialize)]
struct RGB {
    red: u8,
    green: u8,
    blue: u8,
}

// https://tannerhelland.com/2012/09/18/convert-temperature-rgb-algorithm-code.html
fn color_temperature_to_rgb(kelvin: u32) -> RGB {
    let temp = kelvin as f32 / 100.0;

    let red = 
        if kelvin <= 6600 {
            255
        } else {
            ((329.698727446 * ((temp - 60.0).powf(-0.1332047592))) as u32).min(255) as u8
        };

    let green = 
        if kelvin <= 6600 {
            (99.4708025861 * temp.ln() - 161.1195681661) as u8
        } else {
            ((288.1221695283 * ((temp - 60.0).powf(-0.0755148492))) as u32).min(255) as u8
        };

    let blue = 
        if kelvin >= 6600 {
            255
        } else if kelvin <= 1900 {
            0
        } else {
            (138.5177312231 * (temp - 10.0).ln() - 305.0447927307) as u8
        };

    RGB {
        red: red,
        green: green,
        blue: blue,
    }
}

fn set_color(rgb: RGB) -> Result<()> {
    let rgb_json = serde_json::to_string(&rgb)?;
    log::info!("Setting {rgb_json}");
    let fbdevice = "/dev/fb0";
    let mut framebuffer = Framebuffer::new(fbdevice)
        .with_context(|| format!("Could not open framebuffer {fbdevice}"))?;

    let width = framebuffer.var_screen_info.xres;
    let height = framebuffer.var_screen_info.yres;
    let width_height: usize = (width * height).try_into()?;

    log::debug!("fb info: {:?}", framebuffer.var_screen_info.clone());

    let blue = rgb.blue >> (8-framebuffer.var_screen_info.blue.length);
    let red = rgb.red >> (8-framebuffer.var_screen_info.red.length);
    let green = rgb.green >> (8-framebuffer.var_screen_info.green.length);

    let color = 
        ((red as u32) << framebuffer.var_screen_info.red.offset) | 
        ((green as u32) << framebuffer.var_screen_info.green.offset) | 
        ((blue as u32) << framebuffer.var_screen_info.blue.offset);

    let color_width_height = std::iter::repeat(color).take(width_height);        

    let screen_buffer: Vec<u8> = 
        match framebuffer.var_screen_info.bits_per_pixel {
            16 => {
                let buffer = 
                    color_width_height.flat_map(|x| [
                        (x & 0xFF) as u8, 
                        ((x & 0xFF00) >> 8) as u8,
                    ])
                    .collect();

                Ok(buffer)
            }
            32 => {
                let buffer = 
                    color_width_height.flat_map(|x| [
                        (x & 0xFF) as u8, 
                        ((x & 0xFF00) >> 8) as u8,
                        ((x & 0xFF0000) >> 16) as u8,
                        ((x & 0xFF000000) >> 24) as u8,
                    ])
                    .collect();

                Ok(buffer)
            }
            x => Err(anyhow!("Unsupported framebuffer bpp {x}"))
        }?;

    framebuffer.write_frame(&screen_buffer);

    Ok(())
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Discovery {
    // availability: String,
    brightness: bool,
    brightness_scale: u32,
    color_mode: bool,
    command_topic: String,
    //device: String,
    effect: bool,
    effect_list: Vec<String>,
    json_attributes_topic: String,
    max_mireds: u16,
    min_mireds: u16,
    name: Option<String>,
    object_id: String,
    //origin: String,
    schema: String,
    state_topic: String,
    supported_color_modes: Vec<String>,
    unique_id: String,
}

#[derive(PartialEq, Eq, Serialize, Deserialize, Clone, Debug)]
struct State {
    brightness: u32,
    color_mode: String,
    color_temp: u32,
    linkquality: u32,
    state: OnOff,
    update_available: bool,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
enum OnOff {
    On,
    Off,
}

impl<'de> Deserialize<'de> for OnOff {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "ON" => Result::Ok(OnOff::On),
            "OFF" => Result::Ok(OnOff::Off),
            x => Err(serde::de::Error::custom(format!("Invalid on/off value {x}"))),
        }
    }
}

impl Serialize for OnOff {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer {
        let s = match self {
            OnOff::Off => "OFF",
            OnOff::On => "ON",
        };
        serializer.serialize_str(s)
    }
}

#[derive(Deserialize, Clone, Copy, Debug)]
struct StateSet {
    state: OnOff,
    brightness: Option<u32>,
    color_temp: Option<u32>,
}

fn calculate_new_state(set_state: &StateSet, state: &State) -> State {
    State {
        brightness: set_state.brightness.unwrap_or(state.brightness),
        color_mode: state.color_mode.clone(),
        color_temp: set_state.color_temp.unwrap_or(state.color_temp),
        linkquality: state.linkquality,
        state: set_state.state,
        update_available: state.update_available,            
    }
}

fn apply_state(state: &State, displays: Vec<DdcDisplay>) -> Result<Vec<DdcDisplay>> {
    let state_json = serde_json::to_string(state)?;
    log::debug!("Applying state {state_json}");

    match state.state {
        OnOff::Off => {
            let displays1 = set_brightness(&Percentage(0), displays)?;
            let displays2 = set_ddc_power(DdcPower::Off, displays1)?;
            Ok(displays2)
        },
        OnOff::On => {
            let displays1 = set_ddc_power(DdcPower::On, displays)?;
            let v = (f32::from(u8::try_from(state.brightness)?) / f32::from(u8::MAX) * 100.0) as u8;
            let displays2 = set_brightness(&Percentage::new(v)?, displays1)?;
            let kelvin = 1_000_000 / state.color_temp; // color_temp comes as mireds
            set_color(color_temperature_to_rgb(kelvin))?;
            Ok(displays2)
        }
    }
}

#[derive(Clone, Debug)]
struct MqttBroker {
    host: String,
    port: u16,
}

fn put_discovery(client: &mut Client, id: &String, get_topic: &String, set_topic: &String) -> Result<()> {
    let discovery = Discovery {
        //availability: {},
        brightness: true,
        brightness_scale: 254,
        color_mode: true,
        command_topic: set_topic.clone(),
        effect: true,
        effect_list: vec![],
        json_attributes_topic: get_topic.clone(),
        max_mireds: 500,
        min_mireds: MIN_MIREDS.into(), // can't be zero apparently, triggers a div-by-zero exception in home-assistant
        name: Some(id.clone()),
        object_id: id.clone(),
        schema: String::from("json"),
        state_topic: get_topic.clone(),
        supported_color_modes: vec![String::from("color_temp")],
        unique_id: id.clone(),
    };

    let id_clone = id.clone();
    let payload = serde_json::to_string(&discovery)?;
    log::info!("Publishing discovery: {payload}");
    client.publish(format!("homeassistant/light/{id_clone}/light/config"), QoS::AtLeastOnce, true, payload)?;
    Ok(())
}

const MIN_MIREDS: u8 = 155;

fn mqtt(entity_id: u64, mqtt: MqttBroker) -> Result<()> {
    let mqttoptions = MqttOptions::new("test", mqtt.host, mqtt.port);

    let id_str = format!("0x{:016x}", entity_id);
    log::info!("Entity id: {id_str}");

    let (mut client, mut conn) = Client::new(mqttoptions, 10);

    let get_topic = format!("test/{id_str}");
    let set_topic = format!("test/{id_str}/set");
    let state_topic = format!("test/{id_str}/state");

    client.subscribe(&get_topic, QoS::AtMostOnce)?;
    client.subscribe(&set_topic, QoS::AtMostOnce)?;
    client.subscribe(&state_topic, QoS::AtMostOnce)?;

    put_discovery(&mut client, &id_str, &get_topic, &set_topic)?;

    let state = State {
        brightness: 0,
        color_mode: String::from("color_temp"),
        color_temp: MIN_MIREDS.into(),
        linkquality:  255,
        state: OnOff::On,
        update_available: false,
    };

    client.publish(&get_topic, QoS::AtMostOnce, false, serde_json::to_string(&state)?)?;

    #[derive(Clone)]
    struct LoopState {
        state: State,
        loaded_state: bool,
    }

    let initial_state = LoopState {
        state: State {
            brightness: u32::MAX,
            color_mode: String::from("color_temp"),
            color_temp: MIN_MIREDS.into(),
            linkquality: 255,
            state: OnOff::Off,
            update_available: false,
        },
        loaded_state: false,
    };


    conn.iter().try_fold(initial_state, |state, notification| {
        let event = notification.with_context(|| "Connection error")?;
        let new_state = match event {
            rumqttc::Event::Incoming(m) => {
                log::debug!("Incoming: {:?}", m);
                match m {
                    rumqttc::Packet::Publish(p) => {
                        let payload = String::from_utf8(p.payload.to_vec())
                            .with_context(|| "Error reading payload")?;
                        if p.topic == get_topic {
                            log::debug!("Incoming get topic: {:?}", p);
                            Ok(state.clone())
                        } else if p.topic == set_topic {
                            let state_set =  serde_json::from_str::<StateSet>(&payload)
                                .with_context(|| format!("Error deserializing StateSet from {payload}"))?;
                            log::debug!("Incoming set topic: {:?}", state_set);
                            let new_state = calculate_new_state(&state_set, &state.state);
                            Ok(LoopState { state: new_state, loaded_state: state.loaded_state })
                        } else if p.topic == state_topic {
                            if !state.loaded_state {
                                let new_state = serde_json::from_str::<State>(&payload)
                                    .with_context(|| format!("Error deserializing persistent state: {payload}"))?;
                                Ok(LoopState { state: new_state, loaded_state: true })
                            } else {
                                Ok(state.clone())
                            }
                        } else {
                            bail!("Unhandled topic {}", p.topic)
                        }
                    },
                    _ => Ok(state.clone()),
                }
            },
            rumqttc::Event::Outgoing(m) => {
                log::debug!("Outgoing: {:?}", m);
                Ok(state.clone())
            }
        }?;
        if new_state.state != state.state {
            let new_state_msg = serde_json::to_string(&new_state.state)?;
            client.publish(&state_topic, QoS::AtMostOnce, true, new_state_msg.clone())?;
            client.publish(&get_topic, QoS::AtMostOnce, false, new_state_msg.clone())?;
            apply_state(&new_state.state, get_ddc_displays())
                .with_context(|| "Could not apply state")?;
        }
        Ok(new_state)
    }).map(|_| ())
}

#[derive(Clone, Debug)]
struct CmdLine {
    entity_id: u64,
    broker: MqttBroker,
}

fn parse_cmdline() -> CmdLine {
    let entity_id = bpaf::long("entity-id")
        .help("Entity ID e.g. '0xec1bbdfffeb1847f'. If not defined /etc/machine-id will be used if available.")
        .argument::<String>("ID")
        .parse(|id| {
            let id_hex = id.trim_start_matches("0x");
            return u64::from_str_radix(id_hex, 16);
        })
        .fallback_with(|| {
            let machine_id = std::fs::read_to_string("/etc/machine-id")
                .with_context(|| "Could not read /etc/machine-id . Use --entity-id to pass entity ID explicitly.")?;
            let byte_slice: [u8; 16] = machine_id.as_bytes()[0..16].try_into()
                .with_context(|| format!("Error reading /etc/machine-id content: {machine_id}"))?;
            let str_slice = std::str::from_utf8(&byte_slice)?;
            let value = u64::from_str_radix(str_slice, 16)
                .with_context(|| format!("Could not parse value from /etc/machine-id: {str_slice}"))?;
            Ok(value)
        });

    let host = bpaf::long("mqtt-host")
        .help("MQTT host")
        .argument::<String>("MQTT_HOST");

    let port = bpaf::long("mqtt-port")
        .help("MQTT port")
        .argument::<u16>("MQTT_PORT")
        .fallback(1883)
        .display_fallback();

    let broker = construct!(MqttBroker { host, port });

    let parsed_args = construct!(CmdLine {entity_id, broker}).to_options().run();

    return parsed_args;

}

fn get_ddc_displays() -> Vec<DdcDisplay> {
    let mut displays = DdcDisplay::enumerate();
    log::info!("Got {} total DDC displays", displays.len());
    displays.retain_mut(|f| f.update_capabilities().is_ok());
    log::info!("Got {} good DDC displays", displays.len());
    displays
}

fn main() -> Result<()> {
    env_logger::init();
    let parsed_args = parse_cmdline();
    mqtt(parsed_args.entity_id, parsed_args.broker)?;
    Ok(())
}
