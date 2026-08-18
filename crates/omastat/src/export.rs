use crate::{
    report::{self, Lens, UsageReport},
    steam::SteamResolver,
    storage::{AppDayTotals, AppTotals, DayTotals, FocusHeatCell, Storage, TitleTotals},
};
use anyhow::Result;
use chrono::{Datelike, Local, TimeZone, Timelike};
use std::collections::{BTreeMap, HashMap, HashSet};

const PALETTE: [&str; 10] = [
    "#4de8ff", "#8f7aff", "#46d369", "#ffd166", "#ff667d", "#25c2a0", "#ff9f43", "#c084fc",
    "#7dd3fc", "#f472b6",
];

#[derive(Debug, Clone)]
pub struct ExportOptions {
    pub lens: Lens,
    pub offset: i32,
    pub title: Option<String>,
}

pub fn render_html(
    storage: &Storage,
    steam: &mut SteamResolver,
    options: ExportOptions,
) -> Result<String> {
    let report = report::usage_report_for_period(storage, steam, options.lens, options.offset)?;
    let (start_ts, end_ts) = (report.query_start_ts, report.query_end_ts);
    let titles = storage.focused_title_totals_between(start_ts, end_ts, 12)?;
    let daily_apps = storage.focused_app_daily_totals_between(start_ts, end_ts)?;
    let heatmap = storage.focus_heatmap_between(start_ts, end_ts)?;
    let lens_cards = lens_cards(storage, steam)?;
    let page_title = options
        .title
        .unwrap_or_else(|| format!("Omastat Replay - {}", report.period.label));

    Ok(document(
        &page_title,
        &report,
        &daily_apps,
        &heatmap,
        &titles,
        &lens_cards,
    ))
}

fn lens_cards(storage: &Storage, steam: &mut SteamResolver) -> Result<Vec<UsageReport>> {
    Lens::ALL
        .into_iter()
        .map(|lens| report::usage_report_for_period(storage, steam, lens, 0))
        .collect()
}

fn document(
    page_title: &str,
    report: &UsageReport,
    daily_apps: &[AppDayTotals],
    heatmap: &[FocusHeatCell],
    titles: &[TitleTotals],
    lens_cards: &[UsageReport],
) -> String {
    let generated = format_timestamp(report.generated_at);
    let focused = report::format_duration(report.total_focused_seconds);
    let open = report::format_duration(report.total_open_seconds);
    let idle = report::format_duration(report.total_idle_seconds);
    let locked = report::format_duration(report.total_locked_seconds);
    let density = report::percent(ratio(
        report.total_focused_seconds,
        report.total_open_seconds.max(1),
    ));
    let app_count = report
        .rows
        .iter()
        .filter(|row| row.focused_seconds > 0)
        .count();
    let longest_streak = longest_focus_streak(&report.daily);
    let peak_day = report
        .daily
        .iter()
        .max_by_key(|day| day.focused_seconds)
        .filter(|day| day.focused_seconds > 0);
    let app_count_label = app_count.to_string();
    let longest_streak_label = format!("{longest_streak}d");
    let peak_day_label = peak_day
        .map(|day| day.label.clone())
        .unwrap_or_else(|| "none".to_string());
    let peak_day_duration = peak_day
        .map(|day| report::format_duration(day.focused_seconds))
        .unwrap_or_else(|| "no focus".to_string());
    let mut number_card_rows = vec![
        NumberCard::new("Open time", &open, "tracked beside focus"),
        NumberCard::new("Apps", &app_count_label, "with focused time"),
        NumberCard::new(
            "Longest streak",
            &longest_streak_label,
            "focused days in a row",
        ),
        NumberCard::new("Peak day", &peak_day_label, &peak_day_duration),
    ];
    if report.total_idle_seconds > 0 {
        number_card_rows.push(NumberCard::new("Idle excluded", &idle, "session idle"));
    }
    if report.total_locked_seconds > 0 {
        number_card_rows.push(NumberCard::new(
            "Locked excluded",
            &locked,
            "session locked",
        ));
    }
    let number_cards_html = number_cards(&number_card_rows);

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<style>
{css}
</style>
</head>
<body>
<main class="replay">
  <section class="hero">
    <div class="hero-copy">
      <p class="eyebrow">Omastat replay</p>
      <h1>{title}</h1>
      <p class="subhead">{period} - {range} - captured locally - generated {generated}</p>
    </div>
    <div class="hero-card hero-total">
      <small>Focused time</small>
      <strong>{focused}</strong>
      <span>{density} focus density</span>
    </div>
  </section>

  <section class="number-grid" aria-label="By the numbers">
    {number_cards}
  </section>

  <section class="grid grid-main">
    <article class="panel panel-wide">
      <div class="panel-heading">
        <div>
          <span class="kicker">Daily replay</span>
          <h2>Focus by day</h2>
        </div>
        <p>Stacked by app for the selected period.</p>
      </div>
      {stacked_days}
    </article>

    <article class="panel">
      <div class="panel-heading compact">
        <div>
          <span class="kicker">Attention ranking</span>
          <h2>Top apps</h2>
        </div>
      </div>
      {ranked_apps}
    </article>
  </section>

  <section class="grid grid-secondary">
    <article class="panel">
      <div class="panel-heading">
        <div>
          <span class="kicker">App gravity</span>
          <h2>App constellation</h2>
        </div>
        <p>Scale shows focus; outline shows density.</p>
      </div>
      {constellation}
    </article>

    <article class="panel">
      <div class="panel-heading">
        <div>
          <span class="kicker">Focus windows</span>
          <h2>Week x hour heatmap</h2>
        </div>
        <p>Brighter cells mark recurring focus.</p>
      </div>
      {heatmap_chart}
    </article>
  </section>

  <section class="grid grid-tertiary">
    <article class="panel">
      <div class="panel-heading compact">
        <div>
          <span class="kicker">Behavior print</span>
          <h2>Rhythm radar</h2>
        </div>
      </div>
      {radar}
    </article>

    <article class="panel">
      <div class="panel-heading compact">
        <div>
          <span class="kicker">Captured titles</span>
          <h2>Captured moments</h2>
        </div>
      </div>
      {title_rows}
    </article>

    <article class="panel">
      <div class="panel-heading compact">
        <div>
          <span class="kicker">Lenses</span>
          <h2>Period lenses</h2>
        </div>
      </div>
      {lens_cards}
    </article>
  </section>

  <footer>
    <span>Generated from local Omastat data</span>
    <span>Self-contained HTML/SVG replay</span>
  </footer>
</main>
</body>
</html>
"#,
        title = escape_html(page_title),
        css = stylesheet(),
        period = escape_html(&report.period.label),
        range = escape_html(&period_range_label(report)),
        generated = escape_html(&generated),
        focused = escape_html(&focused),
        density = escape_html(&density),
        number_cards = number_cards_html,
        stacked_days = stacked_day_chart(&report.daily, daily_apps, &report.rows),
        ranked_apps = ranked_apps(&report.rows, report.total_focused_seconds),
        constellation = app_constellation(&report.rows, report.total_focused_seconds),
        heatmap_chart = heatmap_chart(heatmap),
        radar = behavior_radar(report, heatmap),
        title_rows = title_rows(titles),
        lens_cards = lens_cards_html(lens_cards),
    )
}

fn stylesheet() -> &'static str {
    r#"
:root {
  color-scheme: dark;
  --bg: #101114;
  --bg-2: #18141d;
  --panel: rgba(27, 27, 31, 0.88);
  --panel-strong: rgba(39, 35, 38, 0.94);
  --ink: #f7f1e8;
  --muted: #b6aa9d;
  --line: rgba(236, 180, 94, 0.24);
  --line-strong: rgba(244, 114, 182, 0.48);
  --cyan: #5ad7ff;
  --green: #46d369;
  --yellow: #f6c453;
  --red: #ff667d;
  --purple: #b28cff;
  --shadow: 0 24px 70px rgba(0, 0, 0, 0.34);
}
* { box-sizing: border-box; }
body {
  margin: 0;
  color: var(--ink);
  background: linear-gradient(135deg, #101114 0%, #161821 48%, #24151b 100%);
  font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  letter-spacing: 0;
}
body::before {
  content: "";
  position: fixed;
  inset: 0;
  pointer-events: none;
  background-image:
    linear-gradient(rgba(255,255,255,0.045) 1px, transparent 1px),
    linear-gradient(90deg, rgba(255,255,255,0.035) 1px, transparent 1px);
  background-size: 36px 36px;
  mask-image: linear-gradient(to bottom, black, transparent 82%);
}
.replay {
  position: relative;
  width: min(1340px, calc(100vw - 32px));
  margin: 0 auto;
  padding: 34px 0 38px;
}
.hero {
  display: grid;
  grid-template-columns: 1fr minmax(280px, 420px);
  gap: 22px;
  min-height: 260px;
  align-items: stretch;
}
.hero-copy, .hero-card, .panel, .number-card {
  border: 1px solid var(--line);
  border-radius: 8px;
  background: linear-gradient(145deg, rgba(39, 35, 38, 0.94), rgba(19, 20, 24, 0.92));
  box-shadow: var(--shadow), inset 0 1px 0 rgba(255,255,255,0.06);
}
.hero-copy {
  padding: 32px;
  display: flex;
  flex-direction: column;
  justify-content: flex-end;
  overflow: hidden;
}
.eyebrow, .kicker, .number-card small, .hero-card small, footer, .mini-label {
  color: var(--muted);
  text-transform: uppercase;
  font-size: 0.72rem;
  font-weight: 900;
  letter-spacing: 0;
}
h1, h2, p { margin: 0; }
h1 {
  max-width: 900px;
  margin-top: 12px;
  font-size: 6.4rem;
  line-height: 0.86;
  letter-spacing: 0;
}
h2 {
  margin-top: 4px;
  font-size: 1.5rem;
  line-height: 1;
}
.subhead {
  margin-top: 18px;
  color: #d7cabd;
  font-size: 1.02rem;
}
.hero-total {
  position: relative;
  min-height: 100%;
  padding: 28px;
  display: flex;
  flex-direction: column;
  justify-content: flex-end;
  background:
    linear-gradient(145deg, rgba(246, 196, 83, 0.24), rgba(244, 114, 182, 0.14)),
    linear-gradient(145deg, rgba(50, 42, 38, 0.96), rgba(20, 21, 29, 0.96));
}
.hero-total::before {
  content: "";
  position: absolute;
  inset: 18px;
  border: 1px dashed rgba(255,255,255,0.18);
}
.hero-total strong {
  position: relative;
  display: block;
  margin: 16px 0 10px;
  font-size: 5.8rem;
  line-height: 0.82;
}
.hero-total span {
  position: relative;
  color: #ffe6b1;
  font-weight: 850;
}
.number-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
  gap: 14px;
  margin-top: 16px;
}
.number-card {
  min-height: 118px;
  padding: 17px;
  overflow: hidden;
}
.number-card strong {
  display: block;
  margin-top: 18px;
  font-size: 2rem;
  line-height: 0.95;
  overflow-wrap: anywhere;
}
.number-card span {
  display: block;
  margin-top: 8px;
  color: #dfd1c3;
  font-weight: 750;
}
.grid {
  display: grid;
  gap: 16px;
  margin-top: 16px;
}
.grid-main { grid-template-columns: minmax(0, 1.5fr) minmax(330px, 0.7fr); }
.grid-secondary { grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); }
.grid-tertiary { grid-template-columns: minmax(280px, 0.8fr) minmax(360px, 1fr) minmax(300px, 0.9fr); }
.panel {
  min-width: 0;
  padding: 18px;
}
.panel-heading {
  display: flex;
  justify-content: space-between;
  gap: 18px;
  align-items: flex-start;
  margin-bottom: 18px;
  padding-bottom: 13px;
  border-bottom: 1px solid var(--line);
}
.panel-heading.compact { margin-bottom: 14px; }
.panel-heading p {
  max-width: 360px;
  color: var(--muted);
  font-size: 0.88rem;
  line-height: 1.35;
}
.chart-frame {
  border: 1px solid rgba(236, 180, 94, 0.18);
  border-radius: 6px;
  background: rgba(10, 11, 14, 0.38);
  padding: 12px;
}
.chart-frame svg { width: 100%; height: auto; display: block; overflow: visible; }
.legend-strip {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 12px;
  margin-top: 12px;
}
.legend-chip {
  display: inline-grid;
  grid-template-columns: 12px auto;
  align-items: center;
  gap: 7px;
  color: #cce8eb;
  font-size: 0.8rem;
  font-weight: 800;
}
.swatch {
  width: 12px;
  height: 12px;
  border-radius: 6px;
  box-shadow: 0 0 16px currentColor;
}
.ranked-list, .title-list, .lens-list {
  display: grid;
  gap: 12px;
}
.rank-row {
  display: grid;
  grid-template-columns: 30px 1fr auto;
  gap: 10px;
  align-items: center;
}
.rank-index {
  width: 30px;
  height: 30px;
  display: grid;
  place-items: center;
  border: 1px solid var(--line-strong);
  color: var(--cyan);
  font-weight: 950;
}
.rank-name, .title-name {
  font-weight: 900;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.rank-meta, .title-app, .lens-meta {
  color: var(--muted);
  font-size: 0.78rem;
  font-weight: 800;
}
.rank-time, .title-time {
  font-weight: 950;
  font-variant-numeric: tabular-nums;
}
.rank-bar {
  grid-column: 2 / -1;
  height: 10px;
  border: 1px solid rgba(255,255,255,0.1);
  background: rgba(255,255,255,0.08);
}
.rank-fill {
  height: 100%;
  box-shadow: 0 0 18px currentColor;
}
.title-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 12px;
  padding-bottom: 12px;
  border-bottom: 1px solid rgba(255,255,255,0.08);
}
.title-row:last-child { border-bottom: 0; padding-bottom: 0; }
.lens-card {
  display: grid;
  grid-template-columns: 76px 1fr;
  gap: 12px;
  align-items: center;
  padding: 12px;
  border-radius: 6px;
  border: 1px solid rgba(255,255,255,0.1);
  background: rgba(255,255,255,0.045);
}
.lens-label {
  color: var(--cyan);
  font-weight: 950;
}
.lens-total {
  font-size: 1.24rem;
  font-weight: 950;
}
footer {
  display: flex;
  justify-content: space-between;
  gap: 16px;
  margin-top: 20px;
  padding: 16px 2px 0;
  border-top: 1px solid var(--line);
}
@media (max-width: 1060px) {
  .hero, .grid-main, .grid-secondary, .grid-tertiary, .number-grid {
    grid-template-columns: 1fr;
  }
  h1 { font-size: 3.6rem; }
  .hero-total strong { font-size: 4rem; }
}
@media print {
  body { background: #101114; }
  .replay { width: 100%; padding: 0; }
  body::before { display: none; }
}
"#
}

struct NumberCard<'a> {
    label: &'a str,
    value: &'a str,
    note: &'a str,
}

impl<'a> NumberCard<'a> {
    fn new(label: &'a str, value: &'a str, note: &'a str) -> Self {
        Self { label, value, note }
    }
}

fn number_cards(cards: &[NumberCard<'_>]) -> String {
    cards
        .iter()
        .map(|card| {
            format!(
                r#"<article class="number-card"><small>{}</small><strong>{}</strong><span>{}</span></article>"#,
                escape_html(card.label),
                escape_html(card.value),
                escape_html(card.note),
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn stacked_day_chart(
    days: &[DayTotals],
    daily_apps: &[AppDayTotals],
    rows: &[AppTotals],
) -> String {
    let visible_days = visible_chart_days(days);
    if visible_days.is_empty() {
        return r#"<div class="chart-frame">No daily data for this period.</div>"#.to_string();
    }

    let top_classes = rows
        .iter()
        .filter(|row| row.focused_seconds > 0)
        .take(6)
        .map(|row| row.app_class.clone())
        .collect::<Vec<_>>();
    let top_set = top_classes.iter().cloned().collect::<HashSet<_>>();
    let mut by_day = BTreeMap::<String, BTreeMap<String, i64>>::new();
    for day in &visible_days {
        by_day.entry(day.date.clone()).or_default();
    }
    for row in daily_apps {
        if !by_day.contains_key(&row.date) {
            continue;
        }
        let bucket = if top_set.contains(&row.app_class) {
            row.app_class.clone()
        } else {
            "Other".to_string()
        };
        *by_day
            .entry(row.date.clone())
            .or_default()
            .entry(bucket)
            .or_default() += row.focused_seconds;
    }

    let max_day = visible_days
        .iter()
        .map(|day| day.focused_seconds.max(0))
        .max()
        .unwrap_or(1)
        .max(1);
    let peak = visible_days
        .iter()
        .enumerate()
        .max_by_key(|(_, day)| day.focused_seconds)
        .map(|(index, day)| (index, *day));
    let width = 1080.0;
    let left = 48.0;
    let top = 34.0;
    let chart_h = 250.0;
    let chart_w = width - left - 30.0;
    let gap = if visible_days.len() > 60 {
        2.0
    } else if visible_days.len() > 32 {
        3.0
    } else {
        5.0
    };
    let bar_w = ((chart_w - gap * (visible_days.len().saturating_sub(1) as f64))
        / visible_days.len() as f64)
        .max(2.0);
    let mut segments = String::new();
    let mut labels = String::new();
    let label_step = (visible_days.len() / 9).max(1);
    let mut series = top_classes.clone();
    series.push("Other".to_string());

    for (index, day) in visible_days.iter().enumerate() {
        let x = left + index as f64 * (bar_w + gap);
        let mut y_cursor = top + chart_h;
        if let Some(buckets) = by_day.get(&day.date) {
            for (series_index, class) in series.iter().enumerate() {
                let seconds = buckets.get(class).copied().unwrap_or(0);
                if seconds <= 0 {
                    continue;
                }
                let h = (seconds as f64 / max_day as f64) * chart_h;
                y_cursor -= h;
                segments.push_str(&format!(
                    r##"<rect x="{x:.2}" y="{y_cursor:.2}" width="{bar_w:.2}" height="{h:.2}" rx="2" fill="{}"><title>{}: {} - {}</title></rect>"##,
                    PALETTE[series_index % PALETTE.len()],
                    escape_html(&day.label),
                    escape_html(&app_name(class)),
                    escape_html(&report::format_duration(seconds)),
                ));
            }
        }

        if index % label_step == 0 || index + 1 == visible_days.len() {
            labels.push_str(&format!(
                r##"<text x="{:.2}" y="326" text-anchor="middle" font-size="12" font-weight="850" fill="#9ab8bd">{}</text>"##,
                x + bar_w / 2.0,
                escape_html(&short_date(&day.date)),
            ));
        }
    }

    let mut annotation = String::new();
    if let Some((index, day)) = peak
        && day.focused_seconds > 0
    {
        let x = left + index as f64 * (bar_w + gap) + bar_w / 2.0;
        let bar_h = (day.focused_seconds as f64 / max_day as f64) * chart_h;
        let y = top + chart_h - bar_h;
        annotation = format!(
            r##"<line x1="{x:.2}" y1="{y:.2}" x2="{x:.2}" y2="18" stroke="#ffd166" stroke-width="2" />
<rect x="{label_x:.2}" y="2" width="164" height="38" rx="4" fill="#ffd166" />
<text x="{text_x:.2}" y="18" font-size="11" font-weight="950" fill="#06181f">Peak: {date}</text>
<text x="{text_x:.2}" y="32" font-size="11" font-weight="850" fill="#06181f">{duration}</text>"##,
            label_x = (x + 8.0).min(width - 174.0),
            text_x = (x + 18.0).min(width - 164.0),
            date = escape_html(&day.label),
            duration = escape_html(&report::format_duration(day.focused_seconds)),
        );
    }

    let legend = series
        .iter()
        .enumerate()
        .filter(|(_, class)| *class == "Other" || top_set.contains(*class))
        .map(|(index, class)| {
            format!(
                r#"<span class="legend-chip"><i class="swatch" style="color:{color};background:{color}"></i>{}</span>"#,
                escape_html(&app_name(class)),
                color = PALETTE[index % PALETTE.len()],
            )
        })
        .collect::<Vec<_>>()
        .join("");

    format!(
        r##"<div class="chart-frame"><svg viewBox="0 0 1080 360" role="img" aria-label="Stacked daily app focus chart">
<line x1="48" y1="284" x2="1050" y2="284" stroke="rgba(255,255,255,0.22)" stroke-width="1" />
<line x1="48" y1="159" x2="1050" y2="159" stroke="rgba(255,255,255,0.10)" stroke-width="1" />
<text x="6" y="42" font-size="12" font-weight="900" fill="#9ab8bd">{max_label}</text>
{segments}
{annotation}
{labels}
</svg></div><div class="legend-strip">{legend}</div>"##,
        max_label = escape_html(&report::format_duration(max_day)),
    )
}

fn ranked_apps(rows: &[AppTotals], total: i64) -> String {
    let max = rows
        .iter()
        .map(|row| row.focused_seconds.max(0))
        .max()
        .unwrap_or(1)
        .max(1);
    let ranked = rows
        .iter()
        .filter(|row| row.focused_seconds > 0)
        .take(8)
        .enumerate()
        .map(|(index, row)| {
            let color = PALETTE[index % PALETTE.len()];
            let width = ratio(row.focused_seconds, max) * 100.0;
            let share = report::percent(ratio(row.focused_seconds, total));
            format!(
                r#"<div class="rank-row">
  <div class="rank-index">{rank}</div>
  <div>
    <div class="rank-name">{name}</div>
    <div class="rank-meta">{share} of focused time</div>
  </div>
  <div class="rank-time">{time}</div>
  <div class="rank-bar"><div class="rank-fill" style="width:{width:.2}%;color:{color};background:{color}"></div></div>
</div>"#,
                rank = index + 1,
                name = escape_html(&report::app_label(&row.app_class)),
                time = escape_html(&report::format_duration(row.focused_seconds)),
            )
        })
        .collect::<Vec<_>>();

    if ranked.is_empty() {
        "No focused app time in this period.".to_string()
    } else {
        format!(r#"<div class="ranked-list">{}</div>"#, ranked.join("\n"))
    }
}

fn app_constellation(rows: &[AppTotals], total: i64) -> String {
    let positions = [
        (295.0, 170.0),
        (170.0, 132.0),
        (420.0, 126.0),
        (228.0, 270.0),
        (484.0, 238.0),
        (102.0, 244.0),
        (514.0, 82.0),
        (78.0, 88.0),
        (350.0, 292.0),
        (575.0, 174.0),
        (270.0, 58.0),
        (390.0, 54.0),
    ];
    let mut bubbles = String::new();
    for (index, row) in rows
        .iter()
        .filter(|row| row.focused_seconds > 0)
        .take(positions.len())
        .enumerate()
    {
        let (x, y) = positions[index];
        let share = ratio(row.focused_seconds, total);
        let density = ratio(row.focused_seconds, row.open_seconds.max(1));
        let radius = 18.0 + share.sqrt() * 86.0;
        let color = PALETTE[index % PALETTE.len()];
        let label = report::app_label(&row.app_class);
        let short = compact_label(&label, if radius > 48.0 { 18 } else { 10 });
        bubbles.push_str(&format!(
            r##"<g>
<circle cx="{x:.2}" cy="{y:.2}" r="{radius:.2}" fill="{color}" fill-opacity="0.68" stroke="{stroke}" stroke-width="{stroke_w:.2}">
  <title>{title}: {focus} focused - {density_label} density</title>
</circle>
<text x="{x:.2}" y="{label_y:.2}" text-anchor="middle" font-size="{font_size:.2}" font-weight="950" fill="#f4fbff">{short}</text>
</g>"##,
            stroke = if density > 0.5 { "#ffd166" } else { "#d8fbff" },
            stroke_w = 1.4 + density * 4.0,
            title = escape_html(&label),
            focus = escape_html(&report::format_duration(row.focused_seconds)),
            density_label = escape_html(&report::percent(density)),
            label_y = y + 4.0,
            font_size = if radius > 56.0 { 15.0 } else { 11.0 },
            short = escape_html(&short),
        ));
    }

    if bubbles.is_empty() {
        return r#"<div class="chart-frame">No app constellation for this period.</div>"#
            .to_string();
    }

    format!(
        r##"<div class="chart-frame"><svg viewBox="0 0 640 360" role="img" aria-label="App constellation bubble chart">
<defs>
  <filter id="glow"><feGaussianBlur stdDeviation="5" result="blur"/><feMerge><feMergeNode in="blur"/><feMergeNode in="SourceGraphic"/></feMerge></filter>
</defs>
<circle cx="320" cy="180" r="146" fill="none" stroke="rgba(77,232,255,0.14)" stroke-width="1" />
<circle cx="320" cy="180" r="96" fill="none" stroke="rgba(77,232,255,0.10)" stroke-width="1" />
<g filter="url(#glow)">{bubbles}</g>
</svg></div>"##
    )
}

fn heatmap_chart(cells: &[FocusHeatCell]) -> String {
    let max = cells
        .iter()
        .map(|cell| cell.focused_seconds.max(0))
        .max()
        .unwrap_or(1)
        .max(1);
    let by_key = cells
        .iter()
        .map(|cell| ((cell.weekday, cell.hour), cell.focused_seconds))
        .collect::<HashMap<_, _>>();
    let labels = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let mut rects = String::new();
    for weekday in 0..7_u32 {
        for hour in 0..24_u32 {
            let value = by_key.get(&(weekday, hour)).copied().unwrap_or(0);
            let x = 58.0 + hour as f64 * 22.0;
            let y = 28.0 + weekday as f64 * 24.0;
            let color = heat_color(ratio(value, max));
            rects.push_str(&format!(
                r##"<rect x="{x:.2}" y="{y:.2}" width="18" height="18" rx="3" fill="{color}">
<title>{day} {hour:02}:00 - {duration}</title></rect>"##,
                day = labels[weekday as usize],
                duration = escape_html(&report::format_duration(value)),
            ));
        }
    }
    let mut hour_labels = String::new();
    for hour in [0, 6, 12, 18, 23] {
        hour_labels.push_str(&format!(
            r##"<text x="{:.2}" y="222" text-anchor="middle" font-size="11" font-weight="850" fill="#9ab8bd">{:02}</text>"##,
            67.0 + hour as f64 * 22.0,
            hour,
        ));
    }
    let day_labels = labels
        .iter()
        .enumerate()
        .map(|(index, label)| {
            format!(
                r##"<text x="8" y="{:.2}" font-size="12" font-weight="900" fill="#9ab8bd">{}</text>"##,
                42.0 + index as f64 * 24.0,
                label,
            )
        })
        .collect::<Vec<_>>()
        .join("");

    format!(
        r##"<div class="chart-frame"><svg viewBox="0 0 610 240" role="img" aria-label="Weekday hour heatmap">
{day_labels}
{rects}
{hour_labels}
<text x="540" y="222" font-size="11" font-weight="850" fill="#9ab8bd">hour</text>
</svg></div>"##
    )
}

fn behavior_radar(report: &UsageReport, heatmap: &[FocusHeatCell]) -> String {
    let app_count = report
        .rows
        .iter()
        .filter(|row| row.focused_seconds > 0)
        .count() as f64;
    let total = report.total_focused_seconds.max(1);
    let top_share = report
        .rows
        .iter()
        .find(|row| row.focused_seconds > 0)
        .map(|row| ratio(row.focused_seconds, total))
        .unwrap_or(0.0);
    let density = ratio(
        report.total_focused_seconds,
        report.total_open_seconds.max(1),
    );
    let streak = ratio(
        longest_focus_streak(&report.daily) as i64,
        report.daily.len().max(1) as i64,
    );
    let weekend = ratio(
        report
            .daily
            .iter()
            .filter_map(|day| {
                chrono::NaiveDate::parse_from_str(&day.date, "%Y-%m-%d")
                    .ok()
                    .map(|date| (date, day))
            })
            .filter(|(date, _)| {
                matches!(date.weekday(), chrono::Weekday::Sat | chrono::Weekday::Sun)
            })
            .map(|(_, day)| day.focused_seconds)
            .sum::<i64>(),
        total,
    );
    let night = ratio(
        heatmap
            .iter()
            .filter(|cell| cell.hour < 6 || cell.hour >= 20)
            .map(|cell| cell.focused_seconds)
            .sum::<i64>(),
        total,
    );
    let values = [
        ("Variety", (app_count / 12.0).min(1.0)),
        ("Density", density),
        ("Top-heavy", top_share),
        ("Streak", streak),
        ("Night", night),
        ("Weekend", weekend),
    ];
    let center = (180.0, 160.0);
    let max_r = 112.0;
    let points = values
        .iter()
        .enumerate()
        .map(|(index, (_, value))| {
            let angle = -std::f64::consts::FRAC_PI_2
                + index as f64 * std::f64::consts::TAU / values.len() as f64;
            (
                center.0 + angle.cos() * max_r * value,
                center.1 + angle.sin() * max_r * value,
            )
        })
        .collect::<Vec<_>>();
    let polygon = points
        .iter()
        .map(|(x, y)| format!("{x:.2},{y:.2}"))
        .collect::<Vec<_>>()
        .join(" ");
    let axes = values
        .iter()
        .enumerate()
        .map(|(index, (label, value))| {
            let angle =
                -std::f64::consts::FRAC_PI_2 + index as f64 * std::f64::consts::TAU / values.len() as f64;
            let x2 = center.0 + angle.cos() * max_r;
            let y2 = center.1 + angle.sin() * max_r;
            let lx = center.0 + angle.cos() * (max_r + 34.0);
            let ly = center.1 + angle.sin() * (max_r + 22.0);
            format!(
                r##"<line x1="{cx:.2}" y1="{cy:.2}" x2="{x2:.2}" y2="{y2:.2}" stroke="rgba(255,255,255,0.16)" />
<text x="{lx:.2}" y="{ly:.2}" text-anchor="middle" font-size="11" font-weight="900" fill="#d6fbff">{label}</text>
<text x="{lx:.2}" y="{value_y:.2}" text-anchor="middle" font-size="10" font-weight="850" fill="#9ab8bd">{percent}</text>"##,
                cx = center.0,
                cy = center.1,
                value_y = ly + 13.0,
                label = escape_html(label),
                percent = escape_html(&report::percent(*value)),
            )
        })
        .collect::<Vec<_>>()
        .join("");

    format!(
        r##"<div class="chart-frame"><svg viewBox="0 0 360 330" role="img" aria-label="Behavior radar">
<circle cx="180" cy="160" r="112" fill="none" stroke="rgba(255,255,255,0.12)" />
<circle cx="180" cy="160" r="74" fill="none" stroke="rgba(255,255,255,0.10)" />
<circle cx="180" cy="160" r="37" fill="none" stroke="rgba(255,255,255,0.08)" />
{axes}
<polygon points="{polygon}" fill="rgba(77,232,255,0.34)" stroke="#4de8ff" stroke-width="3" />
</svg></div>"##
    )
}

fn title_rows(titles: &[TitleTotals]) -> String {
    if titles.is_empty() {
        return r#"<div class="title-list">No title data captured for this period.</div>"#
            .to_string();
    }

    let rows = titles
        .iter()
        .take(9)
        .map(|title| {
            format!(
                r#"<div class="title-row">
  <div>
    <div class="title-app">{}</div>
    <div class="title-name">{}</div>
  </div>
  <div class="title-time">{}</div>
</div>"#,
                escape_html(&report::app_label(&title.app_class)),
                escape_html(&title.title),
                escape_html(&report::format_duration(title.focused_seconds))
            )
        })
        .collect::<Vec<_>>();
    format!(r#"<div class="title-list">{}</div>"#, rows.join("\n"))
}

fn lens_cards_html(reports: &[UsageReport]) -> String {
    let rows = reports
        .iter()
        .map(|report| {
            let top = report
                .rows
                .iter()
                .find(|row| row.focused_seconds > 0)
                .map(|row| report::app_label(&row.app_class))
                .unwrap_or_else(|| "No focus yet".to_string());
            format!(
                r#"<article class="lens-card">
  <div class="lens-label">{}</div>
  <div>
    <div class="lens-total">{}</div>
    <div class="lens-meta">{}</div>
  </div>
</article>"#,
                escape_html(report.lens_label),
                escape_html(&report::format_duration(report.total_focused_seconds)),
                escape_html(&top),
            )
        })
        .collect::<Vec<_>>();
    format!(r#"<div class="lens-list">{}</div>"#, rows.join("\n"))
}

fn longest_focus_streak(days: &[DayTotals]) -> usize {
    let mut longest = 0;
    let mut current = 0;
    for day in days {
        if day.focused_seconds > 0 {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }
    longest
}

fn visible_chart_days(days: &[DayTotals]) -> Vec<&DayTotals> {
    let today = Local::now().format("%Y-%m-%d").to_string();
    days.iter()
        .filter(|day| {
            day.date.as_str() <= today.as_str()
                || day.focused_seconds > 0
                || day.open_seconds > 0
                || day.idle_seconds > 0
                || day.locked_seconds > 0
        })
        .collect()
}

fn heat_color(value: f64) -> String {
    let value = value.clamp(0.0, 1.0);
    if value <= 0.0 {
        return "rgba(255,255,255,0.055)".to_string();
    }
    if value < 0.33 {
        let t = value / 0.33;
        return mix_hex((13, 58, 66), (77, 232, 255), t);
    }
    if value < 0.72 {
        let t = (value - 0.33) / 0.39;
        return mix_hex((77, 232, 255), (70, 211, 105), t);
    }
    let t = (value - 0.72) / 0.28;
    mix_hex((70, 211, 105), (255, 209, 102), t)
}

fn mix_hex(a: (u8, u8, u8), b: (u8, u8, u8), t: f64) -> String {
    let blend = |left: u8, right: u8| left as f64 + (right as f64 - left as f64) * t;
    format!(
        "#{:02x}{:02x}{:02x}",
        blend(a.0, b.0).round() as u8,
        blend(a.1, b.1).round() as u8,
        blend(a.2, b.2).round() as u8
    )
}

fn app_name(app_class: &str) -> String {
    if app_class == "Other" {
        "Other".to_string()
    } else {
        report::app_label(app_class)
    }
}

fn period_range_label(report: &UsageReport) -> String {
    match (
        report.period.start_date.as_deref(),
        report.period.end_date.as_deref(),
    ) {
        (Some(start), Some(end)) if start == end => start.to_string(),
        (Some(start), Some(end)) => format!("{start} to {end}"),
        _ => "Lifetime".to_string(),
    }
}

fn format_timestamp(timestamp: i64) -> String {
    Local
        .timestamp_opt(timestamp, 0)
        .single()
        .map(|time| {
            format!(
                "{:04}-{:02}-{:02} {:02}:{:02}",
                time.year(),
                time.month(),
                time.day(),
                time.hour(),
                time.minute()
            )
        })
        .unwrap_or_else(|| timestamp.to_string())
}

fn short_date(date: &str) -> String {
    chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map(|date| format!("{}/{}", date.month(), date.day()))
        .unwrap_or_else(|_| date.to_string())
}

fn ratio(value: i64, total: i64) -> f64 {
    if total <= 0 {
        0.0
    } else {
        (value.max(0) as f64 / total as f64).clamp(0.0, 1.0)
    }
}

fn compact_label(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    value.chars().take(max_chars).collect::<String>()
}

fn escape_html(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::{ExportOptions, render_html};
    use crate::{
        config::Config,
        report::Lens,
        steam::SteamResolver,
        storage::{IntervalKind, Storage},
    };
    use chrono::Local;

    #[test]
    fn renders_one_page_export_with_escaped_title_data() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("test.db");
        let config = Config::default();
        let storage = Storage::open(Some(&db), &config).unwrap();
        let end = Local::now().timestamp();
        let start = end - 1800;
        let focused = storage
            .start_interval(
                IntervalKind::Focused,
                "firefox",
                None,
                Some("Docs <Dashboard>"),
                start,
            )
            .unwrap();
        storage.close_interval(focused, end).unwrap();

        let mut steam = SteamResolver::default();
        let html = render_html(
            &storage,
            &mut steam,
            ExportOptions {
                lens: Lens::Day,
                offset: 0,
                title: Some("Usage Export".to_string()),
            },
        )
        .unwrap();

        assert!(html.contains("<!doctype html>"));
        assert!(html.contains("Focus by day"));
        assert!(html.contains("Week x hour heatmap"));
        assert!(html.contains("App constellation"));
        assert!(html.contains("Docs &lt;Dashboard&gt;"));
    }
}
