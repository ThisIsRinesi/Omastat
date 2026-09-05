/* global OMastatDomainTrackerConfig, browser, chrome */
(function() {
  var api = typeof browser !== "undefined" ? browser : chrome
  var config = typeof OMastatDomainTrackerConfig !== "undefined"
    ? OMastatDomainTrackerConfig
    : {}
  var hostName = config.hostName || "io.github.thisisrinesi.omastat"
  var appClass = config.appClass || "zen"
  var source = config.source || ("omastat-" + appClass)
  var lastKey = ""
  var lastSentAt = 0

  function queryTabs(query) {
    try {
      var promise = api.tabs.query(query)
      if (promise && typeof promise.then === "function") return promise
    } catch (_) {}
    return new Promise(function(resolve) {
      try {
        api.tabs.query(query, resolve)
      } catch (_) {
        resolve([])
      }
    })
  }

  function sendNativeMessage(message) {
    try { return Promise.resolve(api.runtime.sendNativeMessage(hostName, message)) }
    catch (error) { return Promise.reject(error) }
  }

  function domainFromUrl(url) {
    try {
      var parsed = new URL(url || "")
      if (parsed.protocol !== "http:" && parsed.protocol !== "https:") return ""
      return parsed.hostname.replace(/^www\./i, "").toLowerCase()
    } catch (_) {
      return ""
    }
  }

  var generation = 0

  function sendDomain(domain, reason) {
    var now = Math.floor(Date.now() / 1000)
    var key = appClass + "\n" + domain
    if (key === lastKey && now - lastSentAt < 10) return
    sendNativeMessage({
      type: domain ? "active-domain" : "clear-domain",
      source: source,
      app_class: appClass,
      domain: domain || "",
      timestamp: now,
      reason: reason
    }).then(function(response) {
      if (response && response.ok) { lastKey = key; lastSentAt = now }
    }).catch(function() { lastKey = "" })
  }

  function reportActive(reason) {
    var request = ++generation
    Promise.resolve(api.windows.getLastFocused()).then(function(window) {
      if (request !== generation) return
      if (!window || !window.focused) { sendDomain("", reason); return }
      return queryTabs({ active: true, windowId: window.id }).then(function(tabs) {
        if (request !== generation) return
        sendDomain(tabs && tabs.length ? domainFromUrl(tabs[0].url) : "", reason)
      })
    }).catch(function() { if (request === generation) sendDomain("", reason) })
  }

  api.tabs.onActivated.addListener(function() { reportActive("tab-activated") })
  api.tabs.onUpdated.addListener(function(_tabId, changeInfo) {
    if (changeInfo.url || changeInfo.status === "complete") reportActive("tab-updated")
  })
  api.tabs.onRemoved.addListener(function() { reportActive("tab-removed") })
  api.windows.onFocusChanged.addListener(function() { reportActive("window-focused") })
  if (api.windows.onRemoved) api.windows.onRemoved.addListener(function() { reportActive("window-removed") })
  if (api.runtime.onStartup) api.runtime.onStartup.addListener(function() { reportActive("startup") })
  if (api.runtime.onInstalled) api.runtime.onInstalled.addListener(function() { reportActive("installed") })
  setInterval(function() { reportActive("heartbeat") }, 30000)
  reportActive("loaded")
})()
