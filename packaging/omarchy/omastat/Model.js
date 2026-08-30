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

function fmtDelta(seconds) {
  return (seconds < 0 ? "-" : "+") + fmt(Math.abs(seconds))
}

function excludedDetail(idleSeconds, lockedSeconds, sleepSeconds, unobservedSeconds) {
  var parts = []
  if (Number(idleSeconds || 0) > 0) parts.push("away " + fmt(idleSeconds))
  if (Number(lockedSeconds || 0) > 0) parts.push("locked " + fmt(lockedSeconds))
  if (Number(sleepSeconds || 0) > 0) parts.push("sleep " + fmt(sleepSeconds))
  if (Number(unobservedSeconds || 0) > 0) parts.push("tracker off " + fmt(unobservedSeconds))
  return parts.join("  ")
}

function pausedDetail(idleSeconds, lockedSeconds, sleepSeconds) {
  var parts = []
  if (Number(idleSeconds || 0) > 0) parts.push("away " + fmt(idleSeconds))
  if (Number(lockedSeconds || 0) > 0) parts.push("locked " + fmt(lockedSeconds))
  if (Number(sleepSeconds || 0) > 0) parts.push("sleep " + fmt(sleepSeconds))
  return parts.join("  ")
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

function pad2(n) {
  n = Math.floor(n)
  return n < 10 ? "0" + n : String(n)
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
      app_class: String(rows[j].app_class || ""),
      category: "",
      seconds: seconds,
      open_seconds: Number(rows[j].open_seconds || 0),
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
  var tailOpen = 0
  var total = 0
  for (var i = 0; i < list.length; i++) total += Number(list[i].seconds || 0)
  for (var j = 0; j < list.length; j++) {
    if (j < max - 1) head.push(list[j])
    else {
      tailSeconds += Number(list[j].seconds || 0)
      tailOpen += Number(list[j].open_seconds || 0)
    }
  }
  head.push({
    app: "Other",
    app_class: "Other",
    category: "mixed",
    seconds: tailSeconds,
    open_seconds: tailOpen,
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
  return dateKey(date)
}

function dateKey(date) {
  return date.getFullYear() + "-" + pad2(date.getMonth() + 1) + "-" + pad2(date.getDate())
}

function parseDateKey(key) {
  var parts = String(key || "").split("-")
  if (parts.length !== 3) return null
  var date = new Date(Number(parts[0]), Number(parts[1]) - 1, Number(parts[2]))
  return isNaN(date.getTime()) ? null : date
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

function dayElapsedSeconds(day) {
  return firstNumber(day, ["elapsed_seconds"], 0)
}

function dayObservedSeconds(day) {
  var explicit = optionalNumber(day, "observed_seconds")
  if (explicit !== null) return explicit
  return Math.max(0, dayElapsedSeconds(day) - firstNumber(day, ["unobserved_seconds"], 0))
}

function dayExcludedSeconds(day) {
  var explicit = optionalNumber(day, "excluded_seconds")
  if (explicit !== null) return explicit
  return firstNumber(day, ["idle_seconds"], 0)
    + firstNumber(day, ["locked_seconds"], 0)
    + firstNumber(day, ["sleep_seconds"], 0)
    + firstNumber(day, ["unobserved_seconds"], 0)
}

function relativeDayLabel(day, todayKey) {
  var key = String(day && day.date || "")
  if (key === todayKey) return "Today"
  if (key === previousDateKey(todayKey)) return "Yesterday"
  return String(day && day.label || key || "")
}

function trendDays(daily, todayKey, lens) {
  var list = (daily || []).slice()
  if (String(lens || "day") === "day" && list.length > 7) list = list.slice(list.length - 7)
  var out = []
  for (var i = 0; i < list.length; i++) {
    var focused = dayFocusedSeconds(list[i])
    var open = dayOpenSeconds(list[i])
    var excluded = dayExcludedSeconds(list[i])
    var observed = dayObservedSeconds(list[i])
    if (observed <= 0) observed = focused + excluded
    out.push({
      key: String(list[i].date || ""),
      seconds: focused,
      focused_seconds: focused,
      open_seconds: open,
      elapsed_seconds: dayElapsedSeconds(list[i]),
      excluded_seconds: excluded,
      observed_seconds: observed,
      valueText: fmt(focused),
      excludedText: excluded > 0 ? fmt(excluded) : "0s",
      densityText: observed > 0 ? percent(focused / observed) : "--",
      label: compactDayLabel(list[i], todayKey),
      fullLabel: relativeDayLabel(list[i], todayKey),
      isToday: String(list[i].date || "") === todayKey
    })
  }
  return out
}

function compactDayLabel(day, todayKey) {
  var key = String(day && day.date || "")
  if (key === todayKey) return "Today"
  var label = String(day && day.label || key || "")
  if (label.length <= 6) return label
  var parts = label.split(" ")
  return parts.length > 1 ? parts[0] + " " + parts[1] : label
}

function maxDailySeconds(days) {
  var max = 0
  for (var i = 0; i < (days || []).length; i++) max = Math.max(max, Number(days[i].seconds || 0))
  return max
}

function weekTrendSummary(daily, todayKey) {
  var stats = consistencyStats(daily)
  if (stats.totalDays <= 0) return ""
  var parts = []
  var today = totalForDay(daily, todayKey)
  if (today > 0) parts.push("Today " + fmt(today))
  if (stats.bestDaySeconds > 0) parts.push("Best " + stats.bestDayLabel + " " + fmt(stats.bestDaySeconds))
  parts.push("Average " + fmt(stats.dailyAverageSeconds))
  parts.push(stats.activeDays + "/" + stats.totalDays + " active")
  return parts.join("  ")
}

function trendDefaultText(days) {
  var list = days || []
  if (list.length === 0) return ""
  var best = null
  var total = 0
  var active = 0
  for (var i = 0; i < list.length; i++) {
    var seconds = Number(list[i].seconds || 0)
    total += seconds
    if (seconds > 0) active += 1
    if (!best || seconds > Number(best.seconds || 0)) best = list[i]
  }
  var parts = []
  if (best && Number(best.seconds || 0) > 0) parts.push("Best " + String(best.fullLabel || best.label || "day") + ": " + fmt(best.seconds))
  parts.push("Average " + fmt(list.length > 0 ? Math.round(total / list.length) : 0))
  parts.push(active + "/" + list.length + " active")
  return parts.join("  ")
}

function bestHour(cells) {
  var list = cells || []
  var best = null
  var active = 0
  var total = 0
  for (var i = 0; i < list.length; i++) {
    var seconds = Number(list[i].seconds || 0)
    total += seconds
    if (seconds > 0) active += 1
    if (!best || seconds > Number(best.seconds || 0)) best = list[i]
  }
  if (!best || Number(best.seconds || 0) <= 0) {
    return { label: "--", value: "--", detail: "No focused hours", active: 0, total: list.length }
  }
  return {
    label: String(best.fullLabel || best.label || hourLabel(best.hour || 0)),
    value: fmt(Number(best.seconds || 0)),
    detail: active + "/" + list.length + " active hours",
    active: active,
    total: list.length
  }
}

function consistencyStats(daily) {
  var list = daily || []
  var active = 0
  var total = 0
  var best = null
  var streak = 0
  var longest = 0

  for (var i = 0; i < list.length; i++) {
    var focused = dayFocusedSeconds(list[i])
    total += focused
    if (!best || focused > best.seconds) {
      best = {
        label: String(list[i].label || list[i].date || ""),
        seconds: focused
      }
    }
    if (focused > 0) {
      active += 1
      streak += 1
      longest = Math.max(longest, streak)
    } else {
      streak = 0
    }
  }

  return {
    activeDays: active,
    totalDays: list.length,
    longestStreak: longest,
    dailyAverageSeconds: list.length > 0 ? Math.round(total / list.length) : 0,
    bestDayLabel: best && best.seconds > 0 ? best.label : "--",
    bestDaySeconds: best ? best.seconds : 0
  }
}

function monthCells(daily, lens) {
  var lensValue = String(lens || "month")
  if (lensValue === "year" || lensValue === "life") return weekCells(daily)

  var list = daily || []
  if (list.length === 0) return []
  var out = []
  var first = parseDateKey(list[0].date)
  var leading = first ? (first.getDay() + 6) % 7 : 0
  for (var blank = 0; blank < leading; blank++) out.push({ blank: true, seconds: 0, label: "" })
  for (var i = 0; i < list.length; i++) {
    var date = parseDateKey(list[i].date)
    out.push({
      blank: false,
      date: String(list[i].date || ""),
      day: date ? date.getDate() : i + 1,
      label: String(list[i].label || list[i].date || ""),
      seconds: dayFocusedSeconds(list[i]),
      open_seconds: dayOpenSeconds(list[i]),
      excluded_seconds: dayExcludedSeconds(list[i]),
      elapsed_seconds: dayElapsedSeconds(list[i]),
      observed_seconds: dayObservedSeconds(list[i])
    })
  }
  return out
}

function monthWeekCells(daily) {
  var list = daily || []
  if (list.length === 0) return []
  var weeks = []
  var current = null

  for (var i = 0; i < list.length; i++) {
    var date = parseDateKey(list[i].date)
    var weekKey = ""
    if (date) {
      var weekStart = new Date(date.getFullYear(), date.getMonth(), date.getDate())
      weekStart.setDate(weekStart.getDate() - ((weekStart.getDay() + 6) % 7))
      weekKey = dateKey(weekStart)
    } else {
      weekKey = "week-" + Math.floor(i / 7)
    }

    if (!current || current.key !== weekKey) {
      current = {
        key: weekKey,
        label: "",
        startLabel: String(list[i].label || list[i].date || ""),
        endLabel: String(list[i].label || list[i].date || ""),
        seconds: 0,
        open_seconds: 0,
        observed_seconds: 0,
        excluded_seconds: 0,
        activeDays: 0,
        totalDays: 0
      }
      weeks.push(current)
    }

    var focused = dayFocusedSeconds(list[i])
    current.seconds += focused
    current.open_seconds += dayOpenSeconds(list[i])
    current.observed_seconds += dayObservedSeconds(list[i])
    current.excluded_seconds += dayExcludedSeconds(list[i])
    current.endLabel = String(list[i].label || list[i].date || "")
    current.totalDays += 1
    if (focused > 0) current.activeDays += 1
  }

  for (var j = 0; j < weeks.length; j++) {
    weeks[j].label = compactWeekRangeLabel(weeks[j].startLabel, weeks[j].endLabel)
    weeks[j].valueText = fmt(weeks[j].seconds)
    weeks[j].densityText = weeks[j].observed_seconds > 0 ? percent(weeks[j].seconds / weeks[j].observed_seconds) : "--"
    weeks[j].fullLabel = "Week " + weeks[j].label
  }
  return weeks
}

function compactWeekRangeLabel(startLabel, endLabel) {
  var start = String(startLabel || "")
  var end = String(endLabel || "")
  if (start === end || end.length === 0) return start
  var startParts = start.split(" ")
  var endParts = end.split(" ")
  if (startParts.length > 1 && endParts.length > 1 && startParts[0] === endParts[0]) {
    return startParts[0] + " " + startParts[1] + "-" + endParts[1]
  }
  return start + " - " + end
}

function weekCells(daily) {
  var list = daily || []
  if (list.length === 0) return []
  var out = []
  for (var start = 0; start < list.length; start += 7) {
    var end = Math.min(start + 7, list.length)
    var focused = 0
    var open = 0
    var excluded = 0
    var elapsed = 0
    var observed = 0
    for (var i = start; i < end; i++) {
      focused += dayFocusedSeconds(list[i])
      open += dayOpenSeconds(list[i])
      excluded += dayExcludedSeconds(list[i])
      elapsed += dayElapsedSeconds(list[i])
      observed += dayObservedSeconds(list[i])
    }
    var first = list[start] || {}
    var last = list[end - 1] || first
    out.push({
      blank: false,
      weekly: true,
      date: String(first.date || ""),
      day: "W" + (out.length + 1),
      label: compactRangeLabel(first, last),
      seconds: focused,
      open_seconds: open,
      excluded_seconds: excluded,
      elapsed_seconds: elapsed,
      observed_seconds: observed
    })
  }
  return out
}

function weekdayFocusCells(heatmap) {
  var totals = []
  for (var day = 0; day < 7; day++) totals.push({ weekday: day, label: WEEKDAY_LABELS[day], seconds: 0 })
  for (var i = 0; i < (heatmap || []).length; i++) {
    var item = heatmap[i] || {}
    var weekday = Math.max(0, Math.min(6, Number(item.weekday || 0)))
    totals[weekday].seconds += Number(item.focused_seconds || item.seconds || 0)
  }
  return totals
}

function monthBucketCells(daily) {
  var buckets = {}
  var order = []
  for (var i = 0; i < (daily || []).length; i++) {
    var key = String(daily[i].date || "").substr(0, 7)
    if (key.length !== 7) continue
    if (!buckets[key]) {
      var date = parseDateKey(key + "-01")
      buckets[key] = {
        blank: false,
        monthly: true,
        date: key + "-01",
        day: date ? MONTH_LABELS[date.getMonth()] : key,
        label: date ? MONTH_LABELS[date.getMonth()] + " " + date.getFullYear() : key,
        seconds: 0,
        open_seconds: 0,
        excluded_seconds: 0,
        elapsed_seconds: 0,
        observed_seconds: 0
      }
      order.push(key)
    }
    buckets[key].seconds += dayFocusedSeconds(daily[i])
    buckets[key].open_seconds += dayOpenSeconds(daily[i])
    buckets[key].excluded_seconds += dayExcludedSeconds(daily[i])
    buckets[key].elapsed_seconds += dayElapsedSeconds(daily[i])
    buckets[key].observed_seconds += dayObservedSeconds(daily[i])
  }
  return order.map(function(key) { return buckets[key] })
}

function activityCells(daily, lens) {
  var lensValue = String(lens || "day")
  if (lensValue === "year") return monthBucketCells(daily)
  if (lensValue === "life") return weekCells(daily).slice(-13)
  if (lensValue === "month") return monthCells(daily, lens)
  return trendDays(daily, "", lensValue)
}

function compactRangeLabel(first, last) {
  var firstLabel = String(first && (first.label || first.date) || "")
  var lastLabel = String(last && (last.label || last.date) || "")
  if (firstLabel === lastLabel || lastLabel.length === 0) return firstLabel
  return firstLabel + " - " + lastLabel
}

function maxMonthSeconds(cells) {
  var max = 0
  for (var i = 0; i < (cells || []).length; i++) if (!cells[i].blank) max = Math.max(max, Number(cells[i].seconds || 0))
  return max
}

function monthDefaultText(cells, weekly) {
  var list = cells || []
  var best = null
  for (var i = 0; i < list.length; i++) {
    if (list[i].blank) continue
    if (!best || Number(list[i].seconds || 0) > Number(best.seconds || 0)) best = list[i]
  }
  if (!best || Number(best.seconds || 0) <= 0) return weekly ? "No focused weekly buckets" : "No focused days"
  return (weekly ? "Best week " : "Best day ") + String(best.label || best.date || "") + ": " + fmt(best.seconds)
}

var WEEKDAY_LABELS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
var MONTH_LABELS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]
function weekdayLabels() {
  return WEEKDAY_LABELS
}

function bucketLabels(count) {
  var out = []
  for (var i = 0; i < count; i++) out.push(String(i + 1))
  return out
}

function heatmapCells(heatmap) {
  var values = {}
  for (var i = 0; i < (heatmap || []).length; i++) {
    var item = heatmap[i] || {}
    var weekday = Math.max(0, Math.min(6, Number(item.weekday || 0)))
    var hour = Math.max(0, Math.min(23, Number(item.hour || 0)))
    values[weekday + ":" + hour] = Number(item.focused_seconds || item.seconds || 0)
  }

  var out = []
  for (var day = 0; day < 7; day++) {
    for (var h = 0; h < 24; h++) {
      var key = day + ":" + h
      out.push({
        weekday: day,
        hour: h,
        seconds: values[key] || 0
      })
    }
  }
  return out
}

function hourlyCells(heatmap) {
  var totals = []
  for (var h = 0; h < 24; h++) totals.push({ hour: h, seconds: 0 })
  for (var i = 0; i < (heatmap || []).length; i++) {
    var item = heatmap[i] || {}
    var hour = Math.max(0, Math.min(23, Number(item.hour || 0)))
    totals[hour].seconds += Number(item.focused_seconds || item.seconds || 0)
  }
  return totals
}

function hourlyTrendCells(heatmap) {
  var cells = hourlyCells(heatmap)
  var out = []
  for (var i = 0; i < cells.length; i++) {
    out.push({
      key: "hour-" + cells[i].hour,
      seconds: cells[i].seconds,
      focused_seconds: cells[i].seconds,
      open_seconds: 0,
      excluded_seconds: 0,
      observed_seconds: cells[i].seconds,
      valueText: cells[i].seconds > 0 ? fmt(cells[i].seconds) : "",
      excludedText: "0s",
      densityText: "",
      label: hourLabel(cells[i].hour),
      fullLabel: hourLabel(cells[i].hour),
      isToday: false
    })
  }
  return out
}

function maxHourlySeconds(cells) {
  var max = 0
  for (var i = 0; i < (cells || []).length; i++) max = Math.max(max, Number(cells[i].seconds || 0))
  return max
}

function maxHeatSeconds(cells) {
  var max = 0
  for (var i = 0; i < (cells || []).length; i++) max = Math.max(max, Number(cells[i].seconds || 0))
  return max
}

function heatDefaultText(cells) {
  var list = cells || []
  var best = null
  for (var i = 0; i < list.length; i++) {
    if (!best || Number(list[i].seconds || 0) > Number(best.seconds || 0)) best = list[i]
  }
  if (!best || Number(best.seconds || 0) <= 0) return ""
  return "Peak " + heatCellDetailText(best)
}

function heatLegendHigh(maxSeconds) {
  maxSeconds = Number(maxSeconds || 0)
  return maxSeconds > 0 ? "High " + fmt(maxSeconds) : "High"
}

function heatIntensity(seconds, maxSeconds) {
  seconds = Number(seconds || 0)
  maxSeconds = Number(maxSeconds || 0)
  if (seconds <= 0 || maxSeconds <= 0) return 0
  return Math.max(0.16, Math.min(1, seconds / maxSeconds))
}

function hourLabel(hour) {
  hour = Math.floor(Number(hour) || 0)
  if (hour === 0) return "12A"
  if (hour < 12) return hour + "A"
  if (hour === 12) return "12P"
  return (hour - 12) + "P"
}

function trendDetailText(day) {
  if (!day) return ""
  var parts = []
  var label = String(day.fullLabel || day.label || day.key || "Day")
  parts.push(label + ": " + fmt(Number(day.seconds || 0)) + " focused")
  if (Number(day.excluded_seconds || 0) > 0) parts.push(fmt(day.excluded_seconds) + " not counted")
  return parts.join("  ")
}

function monthCellDetailText(cell) {
  if (!cell || cell.blank) return ""
  var parts = []
  parts.push(String(cell.label || cell.date || "Day") + ": " + fmt(Number(cell.seconds || 0)) + " focused")
  if (Number(cell.excluded_seconds || 0) > 0) parts.push(fmt(cell.excluded_seconds) + " not counted")
  return parts.join("  ")
}

function heatCellDetailText(cell) {
  if (!cell) return ""
  var weekday = WEEKDAY_LABELS[Math.max(0, Math.min(6, Number(cell.weekday || 0)))] || "Day"
  return weekday + " " + hourLabel(cell.hour) + ": " + fmt(Number(cell.seconds || 0)) + " focused"
}

function insights(rows, daily, todayKey, totalSeconds) {
  var total = Number(totalSeconds || 0)
  if (total <= 0) return []

  var out = []
  var apps = appList(rows)
  if (apps.length > 0) {
    out.push({
      label: "Top app",
      value: apps[0].app + " - " + fmt(apps[0].seconds) + " (" + apps[0].pct + "%)",
      category: "apps",
      tone: "neutral",
      detail: "Largest share of focused time in this period."
    })
  }

  var yesterday = totalForDay(daily, previousDateKey(todayKey))
  if (yesterday > 0) {
    out.push({
      label: "Compared with yesterday",
      value: fmtDelta(total - yesterday),
      category: "patterns",
      tone: total >= yesterday ? "positive" : "negative",
      detail: "Compares this period with the previous local day."
    })
  }

  var stats = consistencyStats(daily)
  if (stats.bestDaySeconds > 0) {
    out.push({
      label: "Best day",
      value: stats.bestDayLabel + " - " + fmt(stats.bestDaySeconds),
      category: "patterns",
      tone: "positive",
      detail: "Highest focused day in the loaded history."
    })
  }

  return out
}

function enrichedInsights(baseInsights, apps, daily, heatmap, lens, totalSeconds, totalElapsedSeconds) {
  var out = []
  var seen = {}

  function add(item) {
    if (!item) return
    var label = String(item.label || item.title || "")
    var value = String(item.value || "")
    if (label.length === 0 && value.length === 0) return
    var key = label + "|" + value
    if (seen[key]) return
    seen[key] = true
    out.push({
      label: label,
      value: value,
      detail: String(item.detail || item.explanation || ""),
      category: String(item.category || "patterns"),
      confidence: String(item.confidence || ""),
      tone: String(item.tone || "neutral"),
      supporting: item.supporting && typeof item.supporting === "object" ? item.supporting : {}
    })
  }

  var list = baseInsights || []
  for (var i = 0; i < list.length; i++) add(list[i])

  var generated = generatedInsights(apps, daily, heatmap, lens, totalSeconds, totalElapsedSeconds)
  for (var j = 0; j < generated.length; j++) add(generated[j])

  out.sort(function(left, right) {
    return insightPriorityValue(left) - insightPriorityValue(right)
  })
  return out
}

function generatedInsights(apps, daily, heatmap, lens, totalSeconds, totalElapsedSeconds) {
  var out = []
  var total = Number(totalSeconds || 0)
  if (total <= 0) return out

  var projection = monthProjectionInsight(daily, lens)
  if (projection) out.push(projection)

  var window = primeFocusWindowInsight(heatmap, total)
  if (window) out.push(window)

  var weekday = consistentWeekdayInsight(daily)
  if (weekday) out.push(weekday)

  var anchor = attentionAnchorInsight(apps, total)
  if (anchor) out.push(anchor)

  var volatility = dayVolatilityInsight(daily)
  if (volatility) out.push(volatility)

  var elapsed = Number(totalElapsedSeconds || 0)
  if (elapsed > 0) {
    out.push({
      label: "Calendar capture",
      value: percent(total / elapsed),
      detail: fmt(total) + " focused inside " + fmt(elapsed) + " elapsed.",
      category: "system-signals",
      tone: total / elapsed >= 0.25 ? "positive" : "info"
    })
  }

  return out
}

function monthProjectionInsight(daily, lens) {
  if (String(lens || "") !== "month") return null
  var list = daily || []
  if (list.length < 5) return null
  var lastDate = parseDateKey(list[list.length - 1].date)
  if (!lastDate) return null
  var daysInMonth = new Date(lastDate.getFullYear(), lastDate.getMonth() + 1, 0).getDate()
  var total = 0
  for (var i = 0; i < list.length; i++) total += dayFocusedSeconds(list[i])
  var projected = Math.round(total / list.length * daysInMonth)
  return {
    label: "Projected month",
    value: fmt(projected),
    detail: "Current daily pace across " + list.length + " loaded days.",
    category: "patterns",
    tone: projected >= total ? "info" : "neutral"
  }
}

function primeFocusWindowInsight(heatmap, totalSeconds) {
  var hours = hourlyCells(heatmap)
  var best = null
  var runStart = -1
  var runSeconds = 0
  for (var i = 0; i <= hours.length; i++) {
    var seconds = i < hours.length ? Number(hours[i].seconds || 0) : 0
    if (seconds > 0) {
      if (runStart < 0) runStart = i
      runSeconds += seconds
    } else if (runStart >= 0) {
      var runEnd = i - 1
      if (!best || runSeconds > best.seconds) best = { start: runStart, end: runEnd, seconds: runSeconds }
      runStart = -1
      runSeconds = 0
    }
  }
  if (!best || best.seconds <= 0) return null
  var share = Number(totalSeconds || 0) > 0 ? best.seconds / Number(totalSeconds || 1) : 0
  return {
    label: "Prime window",
    value: hourLabel(best.start) + "-" + hourLabel((best.end + 1) % 24),
    detail: fmt(best.seconds) + " focused, " + percent(share) + " of this period.",
    category: "patterns",
    tone: "info"
  }
}

function consistentWeekdayInsight(daily) {
  var list = daily || []
  if (list.length < 7) return null
  var buckets = []
  for (var i = 0; i < 7; i++) buckets.push({ weekday: i, total: 0, active: 0, days: 0 })
  for (var j = 0; j < list.length; j++) {
    var date = parseDateKey(list[j].date)
    if (!date) continue
    var weekday = (date.getDay() + 6) % 7
    var focused = dayFocusedSeconds(list[j])
    buckets[weekday].total += focused
    buckets[weekday].days += 1
    if (focused > 0) buckets[weekday].active += 1
  }
  var best = null
  for (var k = 0; k < buckets.length; k++) {
    if (buckets[k].days <= 0) continue
    var activeShare = buckets[k].active / buckets[k].days
    var avg = Math.round(buckets[k].total / buckets[k].days)
    if (!best || activeShare > best.activeShare || (activeShare === best.activeShare && avg > best.avg)) {
      best = { weekday: buckets[k].weekday, activeShare: activeShare, avg: avg, days: buckets[k].days }
    }
  }
  if (!best) return null
  return {
    label: "Most reliable day",
    value: WEEKDAY_LABELS[best.weekday] + " - " + percent(best.activeShare),
    detail: fmt(best.avg) + " average across " + best.days + " matching days.",
    category: "patterns",
    tone: best.activeShare >= 0.8 ? "positive" : "info"
  }
}

function attentionAnchorInsight(apps, totalSeconds) {
  var list = apps || []
  if (list.length === 0 || Number(totalSeconds || 0) <= 0) return null
  var top = list[0]
  var share = Number(top.seconds || 0) / Number(totalSeconds || 1)
  if (share < 0.25) return null
  return {
    label: "Attention anchor",
    value: String(top.app || "App"),
    detail: fmt(Number(top.seconds || 0)) + " focused, " + percent(share) + " of the period.",
    category: "apps",
    tone: share >= 0.5 ? "caution" : "info"
  }
}

function dayVolatilityInsight(daily) {
  var values = []
  for (var i = 0; i < (daily || []).length; i++) values.push(dayFocusedSeconds(daily[i]))
  if (values.length < 5) return null
  var total = 0
  for (var j = 0; j < values.length; j++) total += values[j]
  var avg = total / values.length
  if (avg <= 0) return null
  var variance = 0
  for (var k = 0; k < values.length; k++) variance += Math.pow(values[k] - avg, 2)
  var deviation = Math.sqrt(variance / values.length)
  var ratio = deviation / avg
  return {
    label: "Day volatility",
    value: percent(Math.min(1, ratio)),
    detail: "Typical swing is " + fmt(Math.round(deviation)) + " around the daily average.",
    category: "focus-quality",
    tone: ratio > 0.65 ? "caution" : (ratio < 0.30 ? "positive" : "info")
  }
}

function insightGroups(insights) {
  var definitions = [
    { key: "focus-quality", title: "Focus Quality" },
    { key: "patterns", title: "Patterns" },
    { key: "apps", title: "Apps" },
    { key: "system-signals", title: "System Signals" }
  ]
  var groups = []
  for (var i = 0; i < definitions.length; i++) {
    var rows = []
    for (var j = 0; j < (insights || []).length; j++) {
      if (String(insights[j].category || "patterns") === definitions[i].key) rows.push(insights[j])
    }
    if (rows.length > 0) groups.push({ key: definitions[i].key, title: definitions[i].title, rows: rows })
  }
  return groups
}

function insightPriorityValue(item) {
  var tone = String(item && item.tone || "")
  var category = String(item && item.category || "")
  if (tone === "caution" || tone === "negative") return 0
  if (category === "focus-quality") return 1
  if (category === "patterns") return 2
  if (category === "apps") return 3
  if (category === "system-signals") return 4
  return 5
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

function stableAppColors(apps, accentHex) {
  var base = hexToHsl(accentHex)
  var out = []
  for (var i = 0; i < (apps || []).length; i++) {
    var key = String(apps[i].app_class || apps[i].app || i)
    var hash = 0
    for (var j = 0; j < key.length; j++) hash = ((hash << 5) - hash + key.charCodeAt(j)) | 0
    var h = base.s < 12 ? Math.abs(hash) % 360 : (base.h + Math.abs(hash) % 150) % 360
    var s = base.s < 12 ? 42 : Math.max(34, Math.min(76, base.s))
    var l = 42 + Math.abs(hash) % 28
    out.push(hslToHex(h, s, l))
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
