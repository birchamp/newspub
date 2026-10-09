# Installing newpub

Download the file for your computer from the latest release on the repository's **Releases** page. Each download is a single file. Between releases, every CI run of the **Package** workflow also has these files under "Artifacts".

| Your computer | Download | Then |
|---------------|----------|------|
| Windows 10 or 11 | `newpub-<version>-windows-x64-setup.exe` | Double-click it and follow the steps. No administrator password is needed. |
| Mac (Apple silicon or Intel), macOS 11 or later | `newpub-<version>-macos-universal.dmg` | Open it and drag **newpub** onto **Applications**. |
| Linux (most distributions) | `newpub-<version>-linux-x86_64.AppImage` | Make it executable (`chmod +x`), then double-click it. |
| Ubuntu, Debian or Mint | `newpub_<version>_amd64.deb` | Double-click it, or run `sudo apt install ./newpub_<version>_amd64.deb`. |

Other downloads:
- `newpub-<version>-macos-universal.pkg` installs into Applications through the macOS installer.
- `newpub-<version>-windows-x64-portable.zip` runs without installing.
- `newpub-<version>-linux-x86_64.tar.gz` holds the bare program.

## The first time you open it

These installers are not yet signed with a paid publisher certificate, so your computer asks once before it runs newpub.

**Windows:** If a blue "Windows protected your PC" box appears, click **More info**, then **Run anyway**.

**Mac:** macOS says it cannot check newpub for malicious software.
1. Click **Done**.
2. Open **System Settings → Privacy & Security**.
3. Scroll down and click **Open Anyway** next to the message about newpub.
4. Confirm.

After that, newpub opens normally.

**Linux:** Nothing extra.

## What the installers set up

- **Windows:**
  - newpub in the Start menu, and on the desktop if you tick the box;
  - `.npub` files open in newpub;
  - you can remove it in **Settings → Apps**.
- **Mac:**
  - newpub in Applications and Launchpad;
  - to remove it, drag it from Applications to the Bin.
- **Linux (.deb):**
  - newpub in the applications menu;
  - `.npub` files open in newpub;
  - remove it with `sudo apt remove newpub`.

## Requirements

- **Graphics:** newpub draws with OpenGL. Where OpenGL 2 is missing (some virtual machines, old drivers), it uses Direct3D 12 on Windows (including Windows' built-in software renderer), Metal on macOS, or Vulkan on Linux. If neither works, it shows a message saying why.
- **Fonts:** none to install. newpub carries its own fonts and spelling dictionary.
