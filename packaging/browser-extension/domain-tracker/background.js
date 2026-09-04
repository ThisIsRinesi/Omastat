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
    try {
      var promise = api.runtime.sendNativeMessage(hostName, message)
      if (promise && typeof promise.catch === "function") promise.catch(function() {})
    } catch (_) {
      try {
        api.runtime.sendNativeMessage(hostName, message, function() {})
      } catch (_) {}
    }
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

  function sendDomain(domain, reason) {
    if (!domain) return
    var now = Math.floor(Date.now() / 1000)
    var key = appClass + "\n" + domain
    if (key === lastKey && now - lastSentAt < 10) return
    lastKey = key
    lastSentAt = now
    sendNativeMessage({
      type: "active-domain",
      source: source,
      app_class: appClass,
      domain: domain,
      timestamp: now,
      reason: reason
    })
  }

  function reportActive(reason) {
    queryTabs({ active: true, currentWindow: true }).then(function(tabs) {
      if (!tabs || !tabs.length) return
      sendDomain(domainFromUrl(tabs[0].url), reason)
    }).catch(function() {})
  }

  api.tabs.onActivated.addListener(function() {
    reportActive("tab-activated")
  })

  api.tabs.onUpdated.addListener(function(_tabId, changeInfo, tab) {
    if (!tab || !tab.active || !changeInfo.url) return
    sendDomain(domainFromUrl(changeInfo.url), "tab-updated")
  })

  if (api.windows && api.windows.onFocusChanged) {
    api.windows.onFocusChanged.addListener(function(windowId) {
      if (windowId === api.windows.WINDOW_ID_NONE) return
      reportActive("window-focused")
    })
  }

  if (api.runtime.onStartup) api.runtime.onStartup.addListener(function() {
    reportActive("startup")
  })
  if (api.runtime.onInstalled) api.runtime.onInstalled.addListener(function() {
    reportActive("installed")
  })
  reportActive("loaded")
})()
