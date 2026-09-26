/* global NagoriDomainTrackerConfig, browser, chrome */
(function() {
  var api = typeof browser !== "undefined" ? browser : chrome
  var config = typeof NagoriDomainTrackerConfig !== "undefined"
    ? NagoriDomainTrackerConfig
    : {}
  var hostName = config.hostName || "io.github.thisisrinesi.nagori"
  var appClass = config.appClass || "zen"
  var source = config.source || ("nagori-" + appClass)
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

  var delivery = Promise.resolve()

  function sendDomain(domain, reason, audibleDomains) {
    audibleDomains = audibleDomains || []
    var now = Math.floor(Date.now() / 1000)
    var key = appClass + "\n" + domain + "\n" + audibleDomains.join("\n")
    if (key === lastKey && now - lastSentAt < 10) return
    var message = {
      type: domain ? "active-domain" : "clear-domain",
      source: source,
      app_class: appClass,
      domain: domain || "",
      audible_domains: audibleDomains,
      timestamp: now,
      reason: reason
    }
    // Serialize deliveries: a slow earlier native host must not overwrite a pause.
    delivery = delivery.catch(function() {}).then(function() {
      return sendNativeMessage(message)
    }).then(function(response) {
      if (response && response.ok) { lastKey = key; lastSentAt = now }
    }).catch(function() { lastKey = "" })
  }

  function reportActive(reason) {
    var request = ++generation
    Promise.all([api.windows.getLastFocused(), queryTabs({ audible: true })]).then(function(results) {
      if (request !== generation) return
      var window = results[0]
      // Audible includes muted tabs in WebExtensions; explicitly exclude them.
      var domains = Array.from(new Set(results[1].filter(function(tab) {
        return tab.audible && !(tab.mutedInfo && tab.mutedInfo.muted) && !tab.incognito
      }).map(function(tab) { return domainFromUrl(tab.url) }).filter(Boolean))).sort()
      if (!window || !window.focused) { sendDomain("", reason, domains); return }
      return queryTabs({ active: true, windowId: window.id }).then(function(tabs) {
        if (request !== generation) return
        var tab = tabs && tabs[0]
        sendDomain(tab && !tab.incognito ? domainFromUrl(tab.url) : "", reason, domains)
      })
    }).catch(function() { if (request === generation) sendDomain("", reason, []) })
  }

  api.tabs.onActivated.addListener(function() { reportActive("tab-activated") })
  api.tabs.onUpdated.addListener(function(_tabId, changeInfo) {
    if (changeInfo.url || changeInfo.status === "complete" || "audible" in changeInfo || "mutedInfo" in changeInfo) reportActive("tab-updated")
  })
  api.tabs.onRemoved.addListener(function() { reportActive("tab-removed") })
  api.windows.onFocusChanged.addListener(function() { reportActive("window-focused") })
  if (api.windows.onRemoved) api.windows.onRemoved.addListener(function() { reportActive("window-removed") })
  if (api.runtime.onStartup) api.runtime.onStartup.addListener(function() { reportActive("startup") })
  if (api.runtime.onInstalled) api.runtime.onInstalled.addListener(function() { reportActive("installed") })
  setInterval(function() { reportActive("heartbeat") }, 30000)
  reportActive("loaded")
})()
