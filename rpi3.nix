{ config, pkgs, lib, ... }:

# based on https://github.com/wagdav/homelab/blob/23934fb3fa30a45ea4d0d6c8d2376a8d3dd54705/hardware/rp3.nix#L22

{
  imports = [ ];

  hardware.i2c.enable = true;

  nix.settings.max-jobs = lib.mkDefault 4;
  nixpkgs.system = "aarch64-linux";

  sdImage.compressImage = false;

  boot = {
    kernelPackages = pkgs.linuxPackages_6_7;
    kernelModules = [ "bcm2835-v4l2" ];
    loader = {
      grub.enable = false;
      generic-extlinux-compatible.enable = true;
    };
  };

  networking.wireless = {
    enable = true;
    networks.${builtins.getEnv "WIFI_SSID"}.psk = builtins.getEnv "WIFI_KEY";
    interfaces = [ "wlan0" ];
  };

  hardware.enableRedistributableFirmware = true;

  environment.systemPackages = with pkgs; [
    libraspberrypi
  ];

  services.openssh = {
    enable = true;
    settings.PermitRootLogin = "yes";
  };

  # Allow the user to log in as root without a password.
  users.users.root.initialHashedPassword = "";

  # Don't require sudo/root to `reboot` or `poweroff`.
  security.polkit.enable = true;

  # Allow passwordless sudo from nixos user
  security.sudo = {
    enable = true;
    wheelNeedsPassword = false;
  };

  # Automatically log in at the virtual consoles.
  services.getty.autologinUser = "root";

  services.tailscale.enable = true;

  services.journald.extraConfig = ''
    Storage = volatile
    RuntimeMaxFileSize = 10M
  '';

  systemd.services.mqtt-light = 
    let script = pkgs.writers.makeScriptWriter { interpreter = "${pkgs.nushell}/bin/nu"; } "mqtt-light-start" ''
      let gateway = ${pkgs.iproute}/bin/ip route get 8.8.8.8 
        | lines 
        | get 0 
        | parse -r '.* via (?<gateway>[^\s]+) .*'
        | get 0
        | get gateway

      let network = ( $gateway | str replace -r '[0-9]$' '0' ) ++ "/24"

      let mqtt_host = (do { ${pkgs.nmap}/bin/nmap -p 1883 --open $network --noninteractive -oX /dev/stderr --no-stylesheet out> /dev/null } | complete).stderr 
        | str replace '<!DOCTYPE nmaprun>' ' ' 
        | from xml 
        | get content
        | where $it.tag == 'host'
        | get content
        | each {|x| $x | where $it.tag == 'address' | get 0 }
        | get attributes
        | get addr
        | where $it != $gateway
        | get 0

      ${pkgs.mqtt-light}/bin/mqtt-light --mqtt-host $mqtt_host
    '';
    in
  {
    description = "MQTT light";
    wantedBy = [ "multi-user.target" ];
    wants = [ "network-online.target" ];
    after = [ "network-online.target" ];
    serviceConfig = {
      Environment = "RUST_LOG=info";
      ExecStart = "${script}";
      Restart = "always";
      StartLimitIntervalSec = 0;
      RestartSec = 1;
    };
  };

  documentation = {
    enable = false;
    man.enable = false;
    dev.enable = false;
  };

  zramSwap = {
    enable = true;
    algorithm = "zstd";
    memoryPercent = 55;
  };

  nix = {
    package = pkgs.nixVersions.nix_2_19;
    extraOptions = ''
      experimental-features = nix-command flakes
    '';
  };

  fileSystems."/" =
    {
      device = "/dev/disk/by-label/NIXOS_SD";
      fsType = "ext4";
    };

  networking = {
    hostName = "rpi3-mqtt-light";
  };


  system.stateVersion = "23.11";
}