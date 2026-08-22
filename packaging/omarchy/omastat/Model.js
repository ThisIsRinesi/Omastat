function fmt(seconds) {
  seconds = Math.max(0, Math.floor(Number(seconds) || 0))
  if (seconds < 60) return seconds + "s"
  var mins = Math.floor(seconds / 60)
  if (mins < 60) return mins + "m"
  var hours = Math.floor(mins / 60)
  var rest = mins % 60
  return rest === 0 ? hours + "h" : hours + "h " + rest + "m"
}

function percent(value) {
  value = Math.max(0, Math.min(1, Number(value) || 0))
  return Math.round(value * 100) + "%"
}

function fmtWords(seconds) {
  seconds = Math.max(0, Math.floor(Number(seconds) || 0))
  if (seconds <= 0) return "0 SECONDS"
  if (seconds < 60) return seconds + (seconds === 1 ? " SECOND" : " SECONDS")
  var mins = Math.floor(seconds / 60)
  if (mins < 60) return mins + (mins === 1 ? " MINUTE" : " MINUTES")
  var hours = Math.floor(mins / 60)
  var rest = mins % 60
  var out = hours + (hours === 1 ? " HOUR" : " HOURS")
  if (rest > 0) out += " " + rest + " MINUTES"
  return out
}

function fmtDelta(seconds) {
  return (seconds < 0 ? "-" : "+") + fmt(Math.abs(seconds))
}

function optionalNumber(object, key) {
  if (!object || object[key] === undefined || object[key] === null) return null
  var value = Number(object[key])
  if (isNaN(value)) return null
  return Math.max(0, Math.floor(value))
}

function firstNumber(object, keys, fallback) {
  for (var i = 0; i < keys.length; i++) {
    var value = optionalNumber(object, keys[i])
    if (value !== null) return value
  }
  return Math.max(0, Math.floor(Number(fallback) || 0))
}

function appLabel(app) {
  var value = String(app || "App").replace(/^com\./, "").replace(/^org\./, "")
  var parts = value.split(".")
  return parts[parts.length - 1] || value
}

function appList(rows) {
  rows = rows || []
  var total = 0
  for (var i = 0; i < rows.length; i++) total += Number(rows[i].focused_seconds || 0)

  var out = []
  for (var j = 0; j < rows.length; j++) {
    var seconds = Number(rows[j].focused_seconds || 0)
    if (seconds <= 0) continue
    out.push({
      app: appLabel(rows[j].app_class),
      seconds: seconds,
      pct: total > 0 ? Math.round(100 * seconds / total) : 0
    })
  }
  out.sort(function(a, b) { return b.seconds - a.seconds })
  return out
}

var DONUT_MAX_SLICES = 6
function groupedApps(apps, maxSlices) {
  var list = apps || []
  var max = typeof maxSlices === "number" ? maxSlices : DONUT_MAX_SLICES
  if (list.length <= max) return list

  var head = []
  var tailSeconds = 0
  var total = 0
  for (var i = 0; i < list.length; i++) total += Number(list[i].seconds || 0)
  for (var j = 0; j < list.length; j++) {
    if (j < max - 1) head.push(list[j])
    else tailSeconds += Number(list[j].seconds || 0)
  }
  head.push({
    app: "Other",
    seconds: tailSeconds,
    pct: total > 0 ? Math.round(100 * tailSeconds / total) : 0
  })
  return head
}

function previousDateKey(key) {
  var parts = String(key || "").split("-")
  if (parts.length !== 3) return ""
  var date = new Date(Number(parts[0]), Number(parts[1]) - 1, Number(parts[2]))
  if (isNaN(date.getTime())) return ""
  date.setDate(date.getDate() - 1)
  return date.getFullYear() + "-" + pad2(date.getMonth() + 1) + "-" + pad2(date.getDate())
}

function pad2(n) {
  n = Math.floor(n)
  return n < 10 ? "0" + n : String(n)
}

function totalForDay(daily, key) {
  for (var i = 0; i < (daily || []).length; i++) {
    if (String(daily[i].date || "") === key) return dayFocusedSeconds(daily[i])
  }
  return 0
}

function dayFocusedSeconds(day) {
  return firstNumber(day, ["focused_seconds", "focus_seconds", "seconds", "total_focused_seconds"], 0)
}

function dayOpenSeconds(day) {
  return firstNumber(day, ["open_seconds", "total_open_seconds"], 0)
}

function dayExcludedSeconds(day) {
  var explicit = optionalNumber(day, "excluded_seconds")
  if (explicit !== null) return explicit
  return firstNumber(day, ["idle_seconds"], 0)
    + firstNumber(day, ["locked_seconds"], 0)
    + firstNumber(day, ["sleep_seconds"], 0)
    + firstNumber(day, ["unobserved_seconds"], 0)
}

function dayDensity(day) {
  var open = dayOpenSeconds(day)
  return open > 0 ? dayFocusedSeconds(day) / open : 0
}

function relativeDayLabel(day, todayKey) {
  var key = String(day && day.date || "")
  if (key === todayKey) return "Today"
  if (key === previousDateKey(todayKey)) return "Yesterday"
  return String(day && day.label || key || "")
}

function weekTrend(daily, todayKey) {
  var out = []
  var list = daily || []
  for (var i = 0; i < list.length; i++) {
    var focused = dayFocusedSeconds(list[i])
    var open = dayOpenSeconds(list[i])
    var excluded = dayExcludedSeconds(list[i])
    out.push({
      key: String(list[i].date || ""),
      seconds: focused,
      focused_seconds: focused,
      open_seconds: open,
      excluded_seconds: excluded,
      density: open > 0 ? focused / open : 0,
      valueText: fmt(focused),
      openText: open > 0 ? fmt(open) : "--",
      excludedText: excluded > 0 ? fmt(excluded) : "0s",
      densityText: open > 0 ? percent(focused / open) : "--",
      focusShareText: open > 0 ? "Focus " + percent(focused / open) : "Focus --",
      label: String(list[i].label || ""),
      isToday: String(list[i].date || "") === todayKey
    })
  }
  return out
}

function weekTrendSummary(daily, todayKey) {
  var list = weekTrend(daily, todayKey)
  if (list.length === 0) return ""
  var total = 0
  var active = 0
  var best = null
  var today = null
  for (var i = 0; i < list.length; i++) {
    total += list[i].seconds
    if (list[i].seconds > 0) active += 1
    if (!best || list[i].seconds > best.seconds) best = list[i]
    if (list[i].isToday) today = list[i]
  }
  var parts = []
  if (today) parts.push("Today " + fmt(today.seconds))
  if (best && best.seconds > 0) parts.push("Best " + String(best.label || best.key) + " " + fmt(best.seconds))
  parts.push("Average " + fmt(Math.round(total / list.length)))
  if (active !== list.length) parts.push(active + "/" + list.length + " days with focus")
  return parts.join("  ")
}

function insights(rows, daily, todayKey, totalSeconds) {
  var total = Number(totalSeconds || 0)
  if (total <= 0) return []

  var out = []
  var apps = appList(rows)
  if (apps.length > 0) {
    out.push({
      label: "Top app",
      value: apps[0].app + " - " + fmt(apps[0].seconds) + " (" + apps[0].pct + "%)"
    })
  }

  var yesterday = totalForDay(daily, previousDateKey(todayKey))
  if (yesterday > 0) {
    out.push({ label: "Compared with yesterday", value: fmtDelta(total - yesterday) })
  }

  var best = null
  for (var i = 0; i < (daily || []).length; i++) {
    if (!best || dayFocusedSeconds(daily[i]) > dayFocusedSeconds(best))
      best = daily[i]
  }
  if (best && dayFocusedSeconds(best) > 0) {
    out.push({
      label: "Best day (7d)",
      value: relativeDayLabel(best, todayKey) + " - " + fmt(dayFocusedSeconds(best))
    })
  }

  return out
}

function hexToHsl(hex) {
  var match = /^#?([0-9a-fA-F]{6})$/.exec(String(hex || "").replace(/^\s+|\s+$/g, ""))
  if (!match) return { h: 205, s: 75, l: 58 }
  var n = parseInt(match[1], 16)
  var r = ((n >> 16) & 255) / 255
  var g = ((n >> 8) & 255) / 255
  var b = (n & 255) / 255
  var max = Math.max(r, g, b)
  var min = Math.min(r, g, b)
  var h = 0
  var s = 0
  var l = (max + min) / 2
  if (max !== min) {
    var d = max - min
    s = l > 0.5 ? d / (2 - max - min) : d / (max + min)
    if (max === r) h = (g - b) / d + (g < b ? 6 : 0)
    else if (max === g) h = (b - r) / d + 2
    else h = (r - g) / d + 4
    h *= 60
  }
  return { h: h, s: s * 100, l: l * 100 }
}

function hslToHex(h, s, l) {
  h = ((h % 360) + 360) % 360
  s /= 100
  l /= 100
  var c = (1 - Math.abs(2 * l - 1)) * s
  var x = c * (1 - Math.abs((h / 60) % 2 - 1))
  var m = l - c / 2
  var r = 0
  var g = 0
  var b = 0
  if (h < 60) { r = c; g = x }
  else if (h < 120) { r = x; g = c }
  else if (h < 180) { g = c; b = x }
  else if (h < 240) { g = x; b = c }
  else if (h < 300) { r = x; b = c }
  else { r = c; b = x }
  function channel(v) {
    var t = Math.max(0, Math.min(255, Math.round((v + m) * 255)))
    return (t < 16 ? "0" : "") + t.toString(16)
  }
  return "#" + channel(r) + channel(g) + channel(b)
}

function sliceColors(count, accentHex) {
  var base = hexToHsl(accentHex)
  var grayRamp = [50, 70, 32, 82, 40, 62, 28, 76]
  var out = []
  for (var i = 0; i < count; i++) {
    var h = base.h + i * 38
    var l = base.l
    if (base.s < 12) l = grayRamp[i % grayRamp.length]
    else if (i % 2 === 1) l = Math.max(32, Math.min(80, base.l - 14))
    out.push(hslToHex(h, base.s, l))
  }
  return out
}

var ARC_GAP_DEG = 1.5
function arcSegments(apps) {
  var list = apps || []
  var total = 0
  for (var i = 0; i < list.length; i++) total += Number(list[i].seconds || 0)
  var gap = list.length > 1 ? ARC_GAP_DEG : 0
  var angle = -90
  var out = []
  for (var j = 0; j < list.length; j++) {
    var frac = total > 0 ? Number(list[j].seconds || 0) / total : 0
    var sweep = j < list.length - 1 ? Math.max(0, frac * 360 - gap) : frac * 360
    out.push({
      app: list[j].app,
      seconds: list[j].seconds,
      pct: list[j].pct,
      startAngle: angle,
      sweepAngle: sweep
    })
    angle += frac * 360
  }
  return out
}
