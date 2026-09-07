# Omastat

Local app and website activity tracking for **Arch Linux, Hyprland, and Omarchy
Quattro**. Open the bar widget to see where your time went, when you were active,
and which habits keep showing up. No timers, accounts, or cloud service.

![Week of August 24–30: app usage, daily totals, hourly activity, and recurring habits](docs/assets/widget-week.png)

- Browse a day, week, month, year, or your full history.
- Select an app or website to see its usage, visits, and patterns.
- Click a calendar day or chart point to open that period. Your selection follows you.
- Open **How we know** to see the evidence behind a pattern.

<details>
<summary>Month and day views</summary>

![August 2026: daily activity, calendar, and weekly totals](docs/assets/widget-month.png)

![August 30, 2026: app breakdown and hour-by-hour activity](docs/assets/widget-day.png)

</details>

*Screenshots show real activity from August 2026.*

## Install

Requires a running Omarchy Quattro desktop, Git, jq, Cargo/Rust, and a C compiler.
Run as your normal user, without `sudo`:

```bash
git clone https://github.com/ThisIsRinesi/Omastat.git
cd Omastat
./install.sh
```

This installs the backend in `~/.cargo/bin`, starts the tracking service, and adds
the widget to your bar. Tracking starts immediately.

For website breakdowns in Zen or Firefox:

```bash
./install.sh --with-browser
```

The installer includes a Mozilla-signed extension for Firefox 140+ and compatible
Zen versions. Restart your browser and enable **Omastat Domain Tracker**, accepting
the browsing activity permission. Domains go only to your local Omastat app;
the native host automatically distinguishes Zen from Firefox.

If the extension does not appear, open `about:addons`, choose **Install Add-on
From File** from the gear menu, and select:

```text
~/.local/share/omastat/browser-extension/omastat-domain-tracker-firefox.xpi
```

The same signed file works in both browsers. If you use `XDG_DATA_HOME`, use that
directory instead of `~/.local/share`. See [browser setup](docs/USAGE.md) for details.

## Use

Click the bar widget to open the dashboard. Choose a period at the top; use the
arrows for earlier periods and **Today / This week / This month** to return.
Select **All activity** to clear an app or website selection.

Keyboard: **Tab** to move between controls, **Enter/Space** to select, **/** to
search, **R** to refresh, and **Escape** to clear the selection or close.

CLI reports and JSON/CSV exports are also available:

```bash
omastat today
omastat export-data --lens month --offset -1 --format csv --output ~/omastat-august
```

## Privacy

Omastat counts foreground app use. Idle, locked, sleep, and unrecorded time are
excluded; active audio can keep a media session counted. Records stay in a local
SQLite database. Window titles are off by default. The optional browser extension
reports domains, not full URLs or page contents.

See [usage and configuration](docs/USAGE.md) for counting rules, privacy options,
exports, and deleting old records.

## Update or uninstall

Update the backend and widget together from your checkout:

```bash
git pull
./install.sh
```

Remove both, including the optional browser integration:

```bash
./uninstall.sh
```

Your recorded activity and configuration are kept. See [installation details](docs/USAGE.md#installation-and-removal)
for plugin backups and existing Git-managed installs.

[MIT license](LICENSE)
