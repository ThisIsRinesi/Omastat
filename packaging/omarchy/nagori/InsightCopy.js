// Advance one phrase per calendar day; refreshes retain the same wording.
function copyDay(date) {
  var value = String(date || "")
  var parsed = Date.parse(value)
  if (isFinite(parsed)) return Math.floor(parsed / 86400000)
  var hash = 0
  for (var i = 0; i < value.length; i++) hash = (hash * 31 + value.charCodeAt(i)) >>> 0
  return hash
}

function predictionTitle(item, tone, date) {
  var s = item.supporting || {}, name = String(s.app_label || s.activity_key || "this activity")
  var recent = s.routine && s.routine.status === "recent"
  var phrases = recent ? {
    warm: ["There's a chance you might open {name} soon", "You may find your way to {name} soon", "{name} could be coming up soon", "A visit to {name} may be coming up", "You might return to {name} soon", "{name} has been showing up around now", "A little time with {name} may be ahead", "This could be a time for {name}", "{name} might fit into this part of your day", "Your recent rhythm may bring you to {name}", "You may spend some time with {name} soon", "{name} could be next"],
    concise: ["You might open {name} soon", "{name} may be coming up", "Possible visit to {name}", "{name} could be next", "You may return to {name}", "A visit to {name} is possible", "{name} may start soon", "{name} might be near", "You could open {name}", "Possible {name} time", "{name} may be ahead", "You might use {name}"],
    playful: ["Could {name} be next?", "Maybe a little {name} soon?", "{name} might make an appearance", "A visit to {name} could be in the cards", "Your day might have room for {name}", "Is {name} around the corner?", "Perhaps {name} is up next", "{name} may be calling", "A little {name} could be ahead", "Might {name} turn up soon?", "The day may bring you back to {name}", "Maybe it's time for {name} soon"]
  } : {
    warm: ["{name} might be coming up", "You often return to {name} around now", "You may return to {name} soon", "This is often your time for {name}", "You may be heading toward {name}", "{name} usually shows up around now", "A familiar time for {name} is near", "This is often when you open {name}", "{name} may be just ahead", "Your usual {name} time is coming up", "You might return to {name} shortly", "A visit to {name} may be near"],
    concise: ["{name} may be coming up", "You may open {name} soon", "Usual {name} time is near", "{name} is often next", "A familiar time to start {name}", "{name} usually starts around now", "You may return to {name}", "{name} may start soon", "Your {name} window is near", "A {name} visit may be ahead", "A usual time for {name}", "{name} could be next"],
    playful: ["Another visit to {name} could be around the corner", "Could {name} be next?", "This is often when {name} joins the day", "A familiar {name} moment may be near", "{name} might make an appearance", "Your {name} hour is drawing near", "Is {name} on the horizon?", "The day often turns toward {name} now", "Perhaps {name} is up next", "{name} tends to show up about now", "A little {name} may be ahead", "The usual {name} time may be close"]
  }
  var list = phrases[tone] || phrases.warm
  var identity = String(s.activity_kind || "") + ":" + String(s.activity_key || "")
  var hash = 0
  for (var i = 0; i < identity.length; i++) hash = (hash * 31 + identity.charCodeAt(i)) >>> 0
  return list[(hash + copyDay(date)) % list.length].replace("{name}", name)
}

function insightHeading(item, tone, date) {
  if (item.kind === "upcoming-activity") return predictionTitle(item, tone, date)
  var s = item.supporting || {}, name = String(s.app_label || s.activity_key || "this activity")
  var templates = {
    "app-routine": {
      warm: ["A familiar time for {name}", "Your rhythm with {name}", "When you return to {name}", "A regular place for {name}", "A pattern in your {name} visits", "Your usual time with {name}"],
      concise: ["A regular time for {name}", "{name} shows up regularly", "Your {name} rhythm", "A familiar {name} time", "You often use {name}", "{name} has a pattern"],
      playful: ["{name} keeps finding its way into your day", "A familiar moment for {name}", "{name} has found a place in your routine", "Your day often makes room for {name}", "{name} often joins this part of your day", "You and {name} have a rhythm"]
    },
    "audio-companion": {
      warm: ["{name} played alongside your activity", "Audio from {name} in the background", "{name} played during your visits", "Your activity included audio from {name}", "{name} played while you used other apps", "Background playback from {name}"],
      concise: ["{name} played in the background", "Background audio from {name}", "{name} alongside other apps", "{name} played while you used other apps", "{name} accompanied this period", "Audio from {name} overlapped your activity"],
      playful: ["{name} played in the background", "{name} played alongside your app visits", "A little {name} in the background", "{name} played alongside your day", "{name} stayed in the mix", "A background soundtrack from {name}"]
    }
  }
  var tentativeRoutine = s.routine && s.routine.status === "recent" || item.kind === "app-routine" && item.confidence === "low"
  var tentative = {
    warm: ["A possible rhythm with {name}", "{name} may be finding a place in your day", "A recent pattern with {name}", "You may be finding a rhythm with {name}", "A possible regular time for {name}", "{name} might be becoming familiar"],
    concise: ["A possible {name} pattern", "A recent rhythm with {name}", "{name} may be finding a rhythm", "A recent pattern with {name}", "{name} could be settling into a routine", "You might be returning to {name}"],
    playful: ["Could {name} be finding its place?", "{name} may be making a habit of showing up", "A little {name} rhythm might be forming", "Perhaps {name} is settling in", "You and {name} might be finding a groove", "{name} could be making a regular appearance"]
  }
  var choices = item.kind === "app-routine" && tentativeRoutine ? tentative[tone] : templates[item.kind] && templates[item.kind][tone]
  if (!choices) return familyHeading(item, tone, date)
  var key = String(s.activity_kind || "") + ":" + String(s.activity_key || "") + ":" + item.kind
  var hash = 0
  for (var i = 0; i < key.length; i++) hash = (hash * 31 + key.charCodeAt(i)) >>> 0
  return choices[(hash + copyDay(date)) % choices.length].replace("{name}", name)
}

// Neutral family headings leave direction, quantities, and comparisons to the
// evidence sentence. They never infer productivity, intent, or mood.
function familyHeading(item, tone, date) {
  var families = {
    comparison: ["day-comparison", "period-comparison", "same-weekday-pace", "focus-momentum", "focus-anomaly", "app-anomaly", "hour-anomaly"],
    timing: ["usually-active-now", "usual-app-now", "peak-focus-hour", "peak-focus-weekday"],
    sessions: ["stretch-trend", "deep-work-blocks", "app-switch-rate", "fragmented-app"],
    distribution: ["top-app", "focus-density", "app-focus-density", "effective-apps", "best-day", "worst-active-day"],
    workspace: ["strongest-workspace", "workspace-app-affinity"],
    streak: ["current-streak", "longest-streak"],
    handoff: ["app-handoff"]
  }
  var phrases = {
    comparison: {
      warm: ["How your time compares", "A look at your recent time", "Your time, side by side", "A little context for your time", "Looking back at your activity", "Your activity in perspective"],
      concise: ["Time comparison", "Activity comparison", "Recorded time", "Comparing activity", "Time in context", "Activity in context"],
      playful: ["A look over your shoulder", "Your time has a backstory", "Two views of your time", "Let's look back a little", "A little then and now", "Your activity, with context"]
    },
    timing: {
      warm: ["The rhythm of your day", "When your activity gathers", "A familiar part of your schedule", "Your time has a rhythm", "A pattern in your hours", "When you tend to be here"],
      concise: ["Activity timing", "Your active hours", "Timing pattern", "Activity rhythm", "Time of activity", "Your schedule pattern"],
      playful: ["Your hours tell a story", "A rhythm in the clock", "Your day has its moments", "A familiar beat in your day", "The clock has a clue", "A little rhythm in your schedule"]
    },
    sessions: {
      warm: ["How your visits unfold", "A look at your activity stretches", "The shape of your time", "Your visits, a little closer", "How you move through apps", "A closer look at your sessions"],
      concise: ["Session pattern", "Activity stretches", "Visit pattern", "App sessions", "Session detail", "How visits unfold"],
      playful: ["Your time has a shape", "A closer look between visits", "The little chapters of your time", "Following your app visits", "Your sessions tell a story", "A peek at your activity stretches"]
    },
    distribution: {
      warm: ["Where your time went", "A look at your recorded time", "How your time adds up", "Your activity at a glance", "The shape of this period", "A closer look at your time"],
      concise: ["Time breakdown", "Recorded activity", "Activity overview", "Time distribution", "Period detail", "Time at a glance"],
      playful: ["Following the trail of your time", "Your time, in little pieces", "A peek at where time went", "The story in your activity", "Putting your time together", "Your activity has a shape"]
    },
    workspace: {
      warm: ["Where your activity happens", "A look at your workspaces", "Your time across workspaces", "A familiar place for your apps", "How you use your spaces", "Your workspace pattern"],
      concise: ["Workspace activity", "Time by workspace", "Workspace pattern", "Workspace use", "Apps and workspaces", "Workspace detail"],
      playful: ["Your apps have their places", "A tour of your workspaces", "Where your apps spend time", "A little workspace geography", "Your time has a place", "Following your apps across spaces"]
    },
    streak: {
      warm: ["The days you returned", "A run of active days", "Your activity across days", "One active day after another", "Looking at your active days", "Your days of recorded activity"],
      concise: ["Active-day streak", "Consecutive active days", "Activity streak", "Days in a row", "Recorded active days", "Your active-day run"],
      playful: ["One day, then another", "A little string of active days", "Your days line up", "Following the days you returned", "A chain of active days", "The days kept adding up"]
    },
    handoff: {
      warm: ["A familiar next step", "How one visit leads to another", "A pattern between your apps", "The apps you open in sequence", "A familiar order to your visits", "Where you tend to go next"],
      concise: ["App sequence", "Recurring app order", "Between app visits", "Visit sequence", "Next-app pattern", "App transitions"],
      playful: ["One app, then another", "Your apps take turns", "A familiar passing of the baton", "Following your next click", "A little sequence in your day", "Your visits have an order"]
    }
  }
  for (var family in families) {
    if (families[family].indexOf(item.kind) < 0) continue
    var list = phrases[family][tone] || phrases[family].warm
    var key = String(item.kind) + String((item.supporting || {}).activity_key || "")
    var hash = 0
    for (var i = 0; i < key.length; i++) hash = (hash * 31 + key.charCodeAt(i)) >>> 0
    return list[(hash + copyDay(date)) % list.length]
  }
  return String(item.title || "Insight")
}

function insightSummary(item, tone, date) {
  var s = item.supporting || {}, routine = s.routine
  var original = String(item.explanation || item.detail || "")
  if (item.kind !== "upcoming-activity" || !routine) return original
  var name = String(s.app_label || s.activity_key || "this activity")
  var count = Number(s.occurrence_count), total = Number(s.eligible_count)
  if (!(count > 0 && total >= count)) return original
  var unit = ["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"].indexOf(String(routine.cadence || "")) >= 0 ? "comparable weeks" : "comparable days"
  var fact = count + " of " + total + " tracked " + unit
  var choices = {
    warm: ["You started {name} around this time on {fact}.", "Around this time, you opened {name} on {fact}.", "Your history shows {name} starts around now on {fact}.", "This has been a time for {name} on {fact}.", "You began a visit to {name} near this time on {fact}.", "Looking back, {name} started around now on {fact}.", "On {fact}, you opened {name} near this time.", "Your visits to {name} began around now on {fact}.", "{name} was part of this hour on {fact}, starting around now.", "On {fact}, a visit to {name} began near this time.", "You found your way to {name} around now on {fact}.", "The recorded starts for {name} fall near this time on {fact}."],
    concise: ["{name} started around now on {fact}.", "Starts near this time: {fact} for {name}.", "{name}: starts around now on {fact}.", "You opened {name} near this time on {fact}.", "Around this time: {name} starts on {fact}.", "{name} visits began near now on {fact}.", "Recorded near this time: {name} starts on {fact}.", "Starts for {name} match this time on {fact}.", "On {fact}, {name} started near now.", "Near-now starts for {name}: {fact}.", "{name} opened around this time on {fact}.", "On {fact}, you started {name} around now."],
    playful: ["The clock has a clue: you started {name} around now on {fact}.", "A familiar moment: {name} started near this time on {fact}.", "Looking back a little, you opened {name} around now on {fact}.", "{name} entered the picture around this time on {fact}.", "Your history points here: {name} starts near now on {fact}.", "This hour has seen {name} begin on {fact}.", "A little clue from your visits: {name} began around now on {fact}.", "On {fact}, this was about when {name} joined the day.", "The pattern so far: {name} started near now on {fact}.", "Your recorded visits tell us {name} began near this time on {fact}.", "Around this time, {name} made an entrance on {fact}.", "On {fact}, a visit to {name} was starting around now."]
  }
  var list = choices[tone] || choices.warm
  return list[copyDay(date) % list.length].replace("{name}", name).replace("{fact}", fact)
}
