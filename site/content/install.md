+++
title = "Install rtm"
description = "How to install the rtm desktop app and CLI on GNU/Linux, Nix and NixOS, macOS and Windows."

[extra]
# Open with the latest release's download links (templates/page.html).
downloads = true
+++

<!-- A copy of the "Installing" section of README.md (headings one level up). Change both together. -->

Download the files from the latest [GitHub Release](https://github.com/brcha/rtm/releases).
`<version>` below stands for the release version, for example `26.1.0`.

## GNU/Linux (x64)

The debs install on Ubuntu 22.04 and newer and Debian 12 and newer. The RPMs install on Fedora,
RHEL 9 and 10 and their clones (AlmaLinux, Rocky Linux), and openSUSE Leap 16.0 and Tumbleweed.

Desktop app, deb:

1. Download `rtmapp_<version>_amd64.deb`.
2. Run `sudo apt install ./rtmapp_<version>_amd64.deb`. This also installs the XDG desktop portal
   with a file chooser (gtk, gnome or kde), which opening a file needs.
3. Start "Rusty Todo.txt Manager" from the application menu, or run `rtmapp`.

It runs on Wayland; X11 works with the X11 libraries the package suggests.

Desktop app, RPM:

1. Download `rtmapp-<version>-1.x86_64.rpm`.
2. Run `sudo dnf install ./rtmapp-<version>-1.x86_64.rpm` (Fedora, RHEL), or
   `sudo zypper install --allow-unsigned-rpm ./rtmapp-<version>-1.x86_64.rpm` (openSUSE). This also
   installs the XDG desktop portal with a file chooser (gtk, gnome or kde).
3. Start "Rusty Todo.txt Manager" from the application menu, or run `rtmapp`.

Desktop app, AppImage (for other distributions):

1. Download `rtmapp_<version>_amd64.AppImage`.
2. Run `chmod +x rtmapp_<version>_amd64.AppImage`.
3. Run `./rtmapp_<version>_amd64.AppImage`.

CLI:

1. Download `rtmcli_<version>_amd64.deb` or `rtmcli-<version>-1.x86_64.rpm`.
2. Run `sudo apt install ./rtmcli_<version>_amd64.deb`,
   `sudo dnf install ./rtmcli-<version>-1.x86_64.rpm` or
   `sudo zypper install --allow-unsigned-rpm ./rtmcli-<version>-1.x86_64.rpm`.
3. Run `rtmcli --help`.

## Nix and NixOS

The flake provides `rtmcli` and `rtmapp`. From a checkout, build them with `nix build .#rtmcli` and
`nix build .#rtmapp`; without one, install them into your profile with
`nix profile add github:brcha/rtm#rtmapp` (or `#rtmcli`).

`github:brcha/rtm` follows `main`, the development branch. To install a release instead, name a
release tag or a release branch:

- `github:brcha/rtm/v<version>`: that release, which never changes, for example
  `nix profile add github:brcha/rtm/v<version>#rtmapp`.
- `github:brcha/rtm/releases/v<line>`: the branch of a release line, for example `releases/v26.1`
  for the 26.1 releases. It gets the line's fixes as soon as they are merged, before their patch
  release; `nix profile upgrade rtmapp` updates to it.

To try a release without installing it, run it:
`nix run github:brcha/rtm/v<version>#rtmcli -- --help`.

To install them on NixOS from a flake-based configuration:

1. Add the `rtm` input to your system `flake.nix`, following your `nixpkgs`, which must have rustc
   1.95 or later (`nixos-26.05` does). For a release, use one of the references above as its `url`.
2. Add the packages to `environment.systemPackages` (or to `home.packages` with Home Manager):

   ```nix
   {
     inputs = {
       nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
       rtm = {
         url = "github:brcha/rtm";
         inputs.nixpkgs.follows = "nixpkgs";
       };
     };

     outputs = { nixpkgs, rtm, ... }: {
       nixosConfigurations.myhost = nixpkgs.lib.nixosSystem {
         system = "x86_64-linux";
         modules = [
           ./configuration.nix
           ({ pkgs, ... }: {
             environment.systemPackages = with rtm.packages.${pkgs.stdenv.hostPlatform.system}; [
               rtmapp
               rtmcli
             ];
           })
         ];
       };
     };
   }
   ```

3. Run `sudo nixos-rebuild switch --flake .#myhost`, then start "Rusty Todo.txt Manager" from the
   application menu, or run `rtmapp` or `rtmcli --help`.

## macOS (Apple Silicon and Intel)

Universal binaries for macOS 11 (Big Sur) and later. They are signed ad hoc only, not with an Apple
Developer ID, and not notarized, so Gatekeeper blocks them until you allow them.

Desktop app:

1. Download `rtmapp_<version>_universal.dmg` and open it.
2. Drag "Rusty Todo.txt Manager" to Applications.
3. Open the app from Applications. macOS 15 refuses with this dialog, which has no Open button:

   > **“Rusty Todo.txt Manager” Not Opened**
   >
   > Apple could not verify “Rusty Todo.txt Manager” is free of malware that may harm your Mac or
   > compromise your privacy.

   Click **Done**. (Earlier macOS versions word the dialog differently.)
4. Open System Settings > Privacy & Security, scroll down to Security, and click **Open Anyway**
   next to the message about "Rusty Todo.txt Manager".
5. Authenticate, then click **Open** in the dialog that follows. Later launches open normally.

Instead of steps 3 to 5, you can remove the quarantine attribute in Terminal:

```sh
xattr -dr com.apple.quarantine "/Applications/Rusty Todo.txt Manager.app"
```

On macOS 14 and earlier, Control-click the app and choose **Open** also works; macOS 15 removed
that shortcut.

The app's settings are in `~/Library/Application Support/rtm/config.toml`.

CLI:

1. Download `rtmcli-<version>-macos-universal.tar.gz` and extract it:
   `tar -xzf rtmcli-<version>-macos-universal.tar.gz`.
2. Remove the quarantine attribute: `xattr -d com.apple.quarantine rtmcli` ("No such xattr" means
   there was none; carry on).
3. Move it to a directory on your `PATH`, for example
   `sudo mkdir -p /usr/local/bin && sudo mv rtmcli /usr/local/bin/`, or `~/.local/bin` if that is
   on your `PATH`.
4. Run `rtmcli --help`.

## Windows (x64)

The packages are unsigned, so Windows warns before running them.

Desktop app:

1. Download `rtmapp_<version>_x64_en-US.msi` and open it.
2. If SmartScreen shows "Windows protected your PC", click **More info**, then **Run anyway**.
3. Follow the installer. It installs to `Program Files` and needs administrator rights: the UAC
   prompt shows "Unknown publisher"; click **Yes**. The Start Menu and desktop shortcuts can be
   deselected.
4. Start "Rusty Todo.txt Manager" from the Start Menu.

The installer replaces an installed older version, including the Tauri-based releases up to 0.1.3,
and keeps its install folder.

CLI:

1. Download `rtmcli-<version>-x64.exe`. It is a portable binary; there is no installer.
2. Optionally rename it to `rtmcli.exe`.
3. Move it to a folder on your `PATH`, or add its folder to `PATH`: Start > "Edit environment
   variables for your account" > `Path` > **Edit** > **New**.
4. Open a new terminal and run `rtmcli --help`.

ARM64 and 32-bit Windows builds are not currently produced.
