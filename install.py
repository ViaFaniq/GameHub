#!/usr/bin/env python3
import os
import sys
import subprocess
import threading
import shutil
import json
from pathlib import Path

# Modern Installer for GameHub.
# Uses Libadwaita for a professional graphical experience

try:
    import gi
    gi.require_version('Gtk', '4.0')
    gi.require_version('Adw', '1')
    from gi.repository import Gtk, Adw, Gio, GLib, Gdk
except ImportError:
    print("Modern Installer requires 'python-gobject', 'gtk4', and 'libadwaita'.")
    print("Please install these dependencies manually first.")
    sys.exit(1)

APP_NAME = "GameHub"
DESKTOP_FILENAME = "com.github.gamehub.desktop"
ICON_NAME = "com.github.gamehub"

class GameHubInstaller(Adw.Application):
    def __init__(self):
        super().__init__(
            application_id='com.github.gamehub.Installer',
            flags=Gio.ApplicationFlags.FLAGS_NONE
        )
        self.project_dir = Path(__file__).resolve().parent
        self.src_dir = self.project_dir / "src"
        self.install_dir = Path.home() / ".local/share/gamehub"
        self.icon_dir = Path.home() / ".local/share/icons/hicolor/128x128/apps"
        self.bin_dir = Path.home() / ".local/bin"

    def apply_custom_css(self):
        css = """
        window {
            background-color: @window_bg_color;
        }
        statuspage {
            margin: 24px;
        }
        .welcome-title {
            font-size: 2.2em;
            font-weight: 800;
            background-image: linear-gradient(135deg, @accent_color, @accent_bg_color);
            color: @accent_color;
        }
        .pill {
            padding: 12px 32px;
            font-weight: bold;
        }
        .install-banner {
            background-color: alpha(@accent_bg_color, 0.1);
            border-radius: 12px;
            padding: 16px;
        }
        """
        provider = Gtk.CssProvider()
        provider.load_from_data(css.encode())
        Gtk.StyleContext.add_provider_for_display(
            Gdk.Display.get_default(),
            provider,
            Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION
        )

    def do_activate(self):
        # 1. Apply Styles
        self.apply_custom_css()

        # 2. Initialize Window
        self.win = Adw.ApplicationWindow(application=self, title="GameHub Installer")
        self.win.set_default_size(500, 650)

        # 3. Create Stack and Clamp
        self.stack = Adw.ViewStack()
        clamp = Adw.Clamp(maximum_size=1000)
        clamp.set_child(self.stack)
        self.win.set_content(clamp)

        # --- Welcome Page ---
        welcome_page = Adw.StatusPage(
            title="Welcome to GameHub",
            description="A unified, modern game library for Linux.",
            icon_name="applications-games-symbolic"
        )
        start_btn = Gtk.Button(
            label="Begin Installation", 
            halign=Gtk.Align.CENTER, 
            css_classes=["suggested-action", "pill"]
        )
        start_btn.set_margin_top(24)
        start_btn.connect("clicked", lambda x: self.stack.set_visible_child_name("steam_api"))
        
        uninstall_btn = Gtk.Button(
            label="Uninstall",
            halign=Gtk.Align.CENTER,
            css_classes=["destructive-action", "pill"]
        )
        uninstall_btn.set_margin_top(12)
        uninstall_btn.connect("clicked", self.on_start_uninstall)
        
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        box.append(start_btn)
        box.append(uninstall_btn)
        welcome_page.set_child(box)
        self.stack.add_titled(welcome_page, "welcome", "Welcome")

        # --- Steam API Page ---
        self.steam_api_page = Adw.StatusPage(
            title="Steam Integration",
            description="Sync your uninstalled games by providing a Steam Web API Key.",
            icon_name="network-workgroup-symbolic"
        )
        
        steam_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        steam_box.set_halign(Gtk.Align.CENTER)
        
        steam_link = Gtk.LinkButton(label="Get your Steam API Key here", uri="https://steamcommunity.com/dev/apikey")
        steam_box.append(steam_link)
        
        self.steam_entry = Gtk.Entry(placeholder_text="Enter Steam Web API Key")
        self.steam_entry.set_size_request(350, -1)
        steam_box.append(self.steam_entry)
        
        steam_btn_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        steam_btn_box.set_halign(Gtk.Align.CENTER)
        
        next_steam_btn = Gtk.Button(label="Next", css_classes=["suggested-action", "pill"])
        next_steam_btn.connect("clicked", lambda x: self.stack.set_visible_child_name("sgdb_api"))
        steam_btn_box.append(next_steam_btn)
        
        skip_steam_btn = Gtk.Button(label="Skip", css_classes=["pill"])
        skip_steam_btn.connect("clicked", lambda x: self.stack.set_visible_child_name("sgdb_api"))
        steam_btn_box.append(skip_steam_btn)
        
        steam_box.append(steam_btn_box)
        self.steam_api_page.set_child(steam_box)
        self.stack.add_titled(self.steam_api_page, "steam_api", "Steam API")

        # --- SteamGridDB API Page ---
        self.sgdb_api_page = Adw.StatusPage(
            title="Automatic Artwork",
            description="Download high-quality covers automatically using SteamGridDB.",
            icon_name="image-x-generic-symbolic"
        )
        
        sgdb_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        sgdb_box.set_halign(Gtk.Align.CENTER)
        
        sgdb_link = Gtk.LinkButton(label="Get your SteamGridDB API Key here", uri="https://www.steamgriddb.com/profile/api")
        sgdb_box.append(sgdb_link)
        
        self.sgdb_entry = Gtk.Entry(placeholder_text="Enter SteamGridDB API Key")
        self.sgdb_entry.set_size_request(350, -1)
        sgdb_box.append(self.sgdb_entry)
        
        sgdb_btn_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        sgdb_btn_box.set_halign(Gtk.Align.CENTER)
        
        next_sgdb_btn = Gtk.Button(label="Finish Setup", css_classes=["suggested-action", "pill"])
        next_sgdb_btn.connect("clicked", lambda x: self.stack.set_visible_child_name("install"))
        sgdb_btn_box.append(next_sgdb_btn)
        
        skip_sgdb_btn = Gtk.Button(label="Skip", css_classes=["pill"])
        skip_sgdb_btn.connect("clicked", lambda x: self.stack.set_visible_child_name("install"))
        sgdb_btn_box.append(skip_sgdb_btn)
        
        sgdb_box.append(sgdb_btn_box)
        self.sgdb_api_page.set_child(sgdb_box)
        self.stack.add_titled(self.sgdb_api_page, "sgdb_api", "SteamGridDB API")

        # --- Install Page ---
        self.install_page = Adw.StatusPage(
            title="Installing GameHub",
            description="Copying files and setting up environment...",
            icon_name="system-software-install-symbolic"
        )
        
        # Progress Bar and Status Label
        self.progress_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        self.progress_box.set_halign(Gtk.Align.CENTER)
        
        self.progress_bar = Gtk.ProgressBar()
        self.progress_bar.set_size_request(300, -1)
        self.progress_box.append(self.progress_bar)
        
        self.status_label = Gtk.Label(label="Ready to install")
        self.progress_box.append(self.status_label)
        
        self.install_btn = Gtk.Button(
            label="Install to ~/.local/share/gamehub",
            halign=Gtk.Align.CENTER,
            css_classes=["suggested-action", "pill"]
        )
        self.install_btn.connect("clicked", self.on_start_install)
        self.progress_box.append(self.install_btn)

        self.install_page.set_child(self.progress_box)
        self.stack.add_titled(self.install_page, "install", "Install")

        # --- Success Page ---
        self.success_page = Adw.StatusPage(
            title="Installation Complete!",
            description="GameHub has been successfully installed.",
            icon_name="emblem-ok-symbolic"
        )
        launch_btn = Gtk.Button(
            label="Launch GameHub", 
            halign=Gtk.Align.CENTER, 
            css_classes=["suggested-action", "pill"]
        )
        launch_btn.set_margin_top(24)
        launch_btn.connect("clicked", self.on_launch)
        self.success_page.set_child(launch_btn)
        self.stack.add_titled(self.success_page, "success", "Success")

        # --- Uninstall Page ---
        self.uninstall_page = Adw.StatusPage(
            title="Uninstalling GameHub",
            description="Removing application files...",
            icon_name="user-trash-symbolic"
        )
        self.uninstall_progress_bar = Gtk.ProgressBar()
        self.uninstall_progress_bar.set_size_request(300, -1)
        self.uninstall_status_label = Gtk.Label(label="Removing files...")
        
        uninstall_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        uninstall_box.set_halign(Gtk.Align.CENTER)
        uninstall_box.append(self.uninstall_progress_bar)
        uninstall_box.append(self.uninstall_status_label)
        self.uninstall_page.set_child(uninstall_box)
        self.stack.add_titled(self.uninstall_page, "uninstall", "Uninstall")

        # --- Uninstall Success Page ---
        self.uninstall_success_page = Adw.StatusPage(
            title="Uninstalled",
            description="GameHub has been removed from your system.",
            icon_name="emblem-ok-symbolic"
        )
        quit_btn = Gtk.Button(
            label="Quit",
            halign=Gtk.Align.CENTER,
            css_classes=["pill"]
        )
        quit_btn.connect("clicked", lambda x: self.quit())
        self.uninstall_success_page.set_child(quit_btn)
        self.stack.add_titled(self.uninstall_success_page, "uninstalled", "Uninstalled")

        self.win.present()

    def update_status(self, text, fraction):
        GLib.idle_add(self.status_label.set_text, text)
        GLib.idle_add(self.progress_bar.set_fraction, fraction)

    def get_git_version(self):
        try:
            if (self.project_dir / ".git").exists():
                commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=self.project_dir).decode().strip()
                short_commit = subprocess.check_output(["git", "rev-parse", "--short", "HEAD"], cwd=self.project_dir).decode().strip()
                return {"commit": commit, "short_commit": short_commit}
        except Exception:
            pass
        return {"commit": "unknown", "short_commit": "unknown"}

    def on_start_install(self, btn):
        btn.set_sensitive(False)
        self.install_page.set_description("Please wait while we set up GameHub.")
        threading.Thread(target=self.run_install_process, daemon=True).start()

    def run_install_process(self):
        try:
            # 1. Prepare Directory
            self.update_status("Creating directories...", 0.1)
            self.install_dir.mkdir(parents=True, exist_ok=True)
            self.bin_dir.mkdir(parents=True, exist_ok=True)
            self.icon_dir.mkdir(parents=True, exist_ok=True)

            # 2. Copy Source Files
            self.update_status("Copying application files...", 0.2)
            dest_src = self.install_dir / "src"
            if dest_src.exists():
                shutil.rmtree(dest_src)
            shutil.copytree(self.src_dir, dest_src)

            
            # 3. Setup Virtual Environment
            self.update_status("Creating virtual environment...", 0.4)
            venv_dir = self.install_dir / "venv"
            subprocess.run([sys.executable, "-m", "venv", str(venv_dir)], check=True)

            # 4. Install Requirements
            self.update_status("Installing Python dependencies...", 0.6)
            pip_exe = venv_dir / "bin" / "pip"
            req_file = self.project_dir / "requirements.txt"
            if req_file.exists():
                subprocess.run([str(pip_exe), "install", "-r", str(req_file)], check=True)
            else:
                # Fallback installation
                subprocess.run([str(pip_exe), "install", "requests", "psutil", "PyGObject"], check=True)

            # 5. Install Icon
            self.update_status("Installing icon...", 0.8)
            icon_src = self.project_dir / "main.png"
            if icon_src.exists():
                # We rename it to .png for compatibility, even if it's a jpg (most DEs handle it)
                # Or better, if we had a proper png.
                shutil.copy(icon_src, self.icon_dir / "com.github.gamehub.png")

            # 6. Write Version File
            self.update_status("Saving version info...", 0.85)
            version_info = self.get_git_version()
            with open(self.install_dir / "version.json", "w") as f:
                json.dump(version_info, f)

            # 7. Create Launch Script
            self.update_status("Creating launcher script...", 0.9)
            launch_script = self.bin_dir / "gamehub"
            python_exe = venv_dir / "bin" / "python3"
            main_script = self.install_dir / "src" / "main.py"
            
            script_content = f"""#!/bin/bash
export GDK_BACKEND=x11
"{python_exe}" "{main_script}" "$@"
"""
            launch_script.write_text(script_content)
            os.chmod(launch_script, 0o755)

            # 7. Create Desktop Entry
            self.create_desktop_file()

            # 8. Post-Install Hooks (Refresh Cache)
            self.update_status("Refreshing system caches...", 0.95)
            try:
                subprocess.run(["update-desktop-database", str(Path.home() / ".local/share/applications")], check=False)
                subprocess.run(["gtk-update-icon-cache", "-f", "-t", str(Path.home() / ".local/share/icons/hicolor")], check=False)
            except Exception:
                pass

            # 9. Save API Keys
            self.update_status("Saving API configurations...", 0.98)
            self.save_api_configs()

            self.update_status("Installation Complete!", 1.0)
            GLib.idle_add(lambda: self.stack.set_visible_child_name("success"))

        except Exception as e:
            self.update_status(f"Error: {e}", 0.0)
            GLib.idle_add(lambda: self.install_btn.set_sensitive(True))
            print(f"Installation failed: {e}")

    def create_desktop_file(self):
        desktop_dir = Path.home() / ".local/share/applications"
        desktop_dir.mkdir(parents=True, exist_ok=True)
        desktop_path = desktop_dir / DESKTOP_FILENAME
        
        # Exec points to the launcher script in ~/.local/bin/gamehub
        exec_path = self.bin_dir / "gamehub"
        
        # Use absolute path for icon to ensure it shows up everywhere (file manager, etc)
        icon_path = self.icon_dir / "com.github.gamehub.png"
        
        content = f"""[Desktop Entry]
Name={APP_NAME}
Comment=Unified game library for Linux
Exec="{exec_path}"
Icon={icon_path}
Terminal=false
Type=Application
Categories=Game;Utility;
StartupNotify=true
StartupWMClass=com.github.gamehub
"""
        desktop_path.write_text(content)
        os.chmod(desktop_path, 0o755)

    def save_api_configs(self):
        steam_key = self.steam_entry.get_text().strip()
        sgdb_key = self.sgdb_entry.get_text().strip()
        
        if not steam_key and not sgdb_key:
            return

        # Add src to path to import SettingsManager
        sys.path.append(str(self.src_dir))
        try:
            from backend.settings_manager import SettingsManager
            settings = SettingsManager()
            
            if steam_key:
                settings.set("steam_api_key", steam_key)
                settings.set("enable_steam_api", "True")
            if sgdb_key:
                settings.set("sgdb_api_key", sgdb_key)
        except Exception as e:
            print(f"Failed to use SettingsManager: {e}")
            # Fallback to manual save if SettingsManager fails (e.g. during dev/partial installs)
            import configparser
            config_dir = Path.home() / ".config/gamehub"
            config_dir.mkdir(parents=True, exist_ok=True)
            settings_file = config_dir / "configuration.conf"
            
            config = configparser.ConfigParser(interpolation=None)
            if settings_file.exists():
                config.read(settings_file)
            if "Settings" not in config:
                config["Settings"] = {}
            if steam_key:
                config["Settings"]["steam_api_key"] = steam_key
                config["Settings"]["enable_steam_api"] = "True"
            if sgdb_key:
                config["Settings"]["sgdb_api_key"] = sgdb_key
            with open(settings_file, "w") as f:
                config.write(f)

    def on_launch(self, btn):
        subprocess.Popen([str(self.bin_dir / "gamehub")])
        self.quit()

    def on_start_uninstall(self, btn):
        dialog = Adw.MessageDialog(
            transient_for=self.win,
            heading="Uninstall GameHub?",
            body="This will remove the application and all its data from your system. Are you sure?",
        )
        dialog.add_response("cancel", "Cancel")
        dialog.add_response("uninstall", "Uninstall")
        dialog.set_response_appearance("uninstall", Adw.ResponseAppearance.DESTRUCTIVE)
        
        def response_cb(dialog, response):
            if response == "uninstall":
                self.stack.set_visible_child_name("uninstall")
                threading.Thread(target=self.run_uninstall_process, daemon=True).start()
        
        dialog.connect("response", response_cb)
        dialog.present()

    def update_uninstall_status(self, text, fraction):
        GLib.idle_add(self.uninstall_status_label.set_text, text)
        GLib.idle_add(self.uninstall_progress_bar.set_fraction, fraction)

    def run_uninstall_process(self):
        try:
            # 1. Remove Desktop Entry
            self.update_uninstall_status("Removing desktop entry...", 0.2)
            desktop_file = Path.home() / ".local/share/applications" / DESKTOP_FILENAME
            if desktop_file.exists():
                desktop_file.unlink()

            # 2. Remove Icon
            self.update_uninstall_status("Removing icon...", 0.4)
            icon_file = self.icon_dir / "com.github.gamehub.png"
            if icon_file.exists():
                icon_file.unlink()

            # 3. Remove Launch Script
            self.update_uninstall_status("Removing launcher script...", 0.6)
            launch_script = self.bin_dir / "gamehub"
            if launch_script.exists():
                launch_script.unlink()

            # 4. Remove Installation Directory
            self.update_uninstall_status("Removing application files...", 0.8)
            if self.install_dir.exists():
                shutil.rmtree(self.install_dir)

            # 5. Refresh Caches
            self.update_uninstall_status("Refreshing system caches...", 0.9)
            try:
                subprocess.run(["update-desktop-database", str(Path.home() / ".local/share/applications")], check=False)
                subprocess.run(["gtk-update-icon-cache", "-f", "-t", str(Path.home() / ".local/share/icons/hicolor")], check=False)
            except Exception:
                pass

            self.update_uninstall_status("Uninstallation Complete", 1.0)
            GLib.idle_add(lambda: self.stack.set_visible_child_name("uninstalled"))

        except Exception as e:
            self.update_uninstall_status(f"Error: {e}", 0.0)
            print(f"Uninstall failed: {e}")

if __name__ == "__main__":
    app = GameHubInstaller()
    app.run(sys.argv)
