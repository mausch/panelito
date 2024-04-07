# Panelito

Panelito turns any Linux machine with a screen into a light that can be controlled with [Home Assistant](https://www.home-assistant.io/).

## Why?

Mainly I just needed a small project to learn Rust.<br>
But also I love LED panels for lighting. You can get very cheap 600x600 panels but they're usually for office settings so they're bright white and not dimmable. The more versatile panels for a home are dimmable, support "[tunable white](https://leddynamics.com/a-guide-to-tunable-white-led-lighting)" i.e. the ability to change the colour temperature output, and integrate easily with Home Assistant.<br>
Unfortunately those are hard to come by and tend to be expensive. I have a half-broken monitor and a cheap Raspberry Pi around so I figured I might be able to build my own. Cheap used half-broken TVs can be easily found either on Craigslist or Gumtree.<br>
However the main downside of using a monitor for lighting is that monitors aren't usually very bright though, unless you happen to have an HDR display, but those aren't common or cheap yet, plus HDR is not supported by this program at the moment.

## How to use

Requirements:

- [Home Assistant](https://www.home-assistant.io/)
- [MQTT integration](https://www.home-assistant.io/integrations/mqtt/)
- Some MQTT broker obviously, for example [Mosquitto](https://mosquitto.org/)
- i2c-dev kernel module loaded

`panelito --mqtt-host <IP>`

A new light will be registered in Home Assistant offering the following controls:

- On/off
- Brightness
- Colour temperature ("tunable white")


## Building / developing

Install [Nix](https://github.com/DeterminateSystems/nix-installer?tab=readme-ov-file#the-determinate-nix-installer), then:


You can build and run it:

```
nix run github:mausch/panelito -- --mqtt-host <IP>
```

Get a development environment:

```
git clone https://github.com/mausch/panelito.git
cd panelito
nix develop
```

Build and burn a Raspberry Pi 3B+ SD card image (you'll need qemu if you're on x86):


```
WIFI_SSID='<YOUR_WIFI>' WIFI_KEY='<YOUR_WIFI_PASSWORD' nix build --impure github:mausch/panelito#rpi3-sdcard
dd bs=5M status=progress if=$(ls result/sd-image/nixos-sd-image-*.img) of=/dev/mmcblk0
```

For some reason the SD card reader module fails sometimes on my laptop so I need to reload it:

```
rmmod rtsx_pci_sdmmc rtsx_pci
modprobe rtsx_pci_sdmmc
```