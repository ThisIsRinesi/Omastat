import QtQuick
import QtQuick.Window
import Quickshell

// Exercise the production panel with deterministic data, without a desktop shell.
ShellRoot {
  Window {
    id: win
    visible: true
    width: 380
    height: 480
    color: "#181820"
    property string output: OUTPUT_PATH
    property int step: -1
    property int mode: 0
    property var cases: []
    Rectangle { anchors.fill: parent; color: win.mode === 2 ? "black" : win.color }
    Panel {
      id: panel
      anchors.fill: parent
      settings: ({panelWidth: win.width, reduceMotion: true, richGraphs: win.mode !== 1, dynamicIslandStyle: win.mode === 2})
      periodLabel: "September 21–27, 2026"
      selectedOffset: -1
    }
    Component.onCompleted: {
      for (var w of [380, 760, 1160, 2000])
        for (var lens of ['day', 'week', 'month', 'year', 'life'])
          for (var mode of [0, 1, 2])
            for (var state of ['populated', 'empty', 'selected', 'loading', 'error', 'settings', 'evidence'])
              cases.push({width: w, lens: lens, mode: mode, state: state})
      panel.open()
      advance()
    }
    function advance() {
      step++
      if (step >= cases.length) { Qt.quit(); return }
      var c = cases[step]
      width = c.width
      mode = c.mode
      panel.selectedLens = c.lens
      panel.selectedActivityKind = c.state === 'selected' ? 'domain' : ''
      panel.selectedActivityKey = c.state === 'selected' ? 'very-long-domain-name-for-layout-checks.example.com' : ''
      panel.activityType = c.state === 'selected' ? 'domain' : 'app'
      panel.dataExpanded = c.state === 'evidence'
      panel.appearanceOpen = c.state === 'settings'
      panel.panelDataLoaded = c.state !== 'loading'
      panel.refreshRunning = c.state === 'loading'
      panel.errorText = c.state === 'error' ? 'Could not refresh activity. Check the tracker and try again.' : ''
      var empty = c.state === 'empty' || c.state === 'loading'
      var count = c.lens === 'day' ? 1 : c.lens === 'week' ? 7 : c.lens === 'month' ? 30 : 365
      panel.daily = Array.from({length: count}, function(_, i) {
        return {date: new Date(2026, c.lens === 'year' || c.lens === 'life' ? 0 : 8, i + 1).toISOString().slice(0,10), focused_seconds: empty ? 0 : 3600 * (i % 6)}
      })
      panel.heatmap = empty ? [] : Array.from({length:168}, function(_, i) { return {weekday:Math.floor(i/24),hour:i%24,focused_seconds:300*(i%7)} })
      var activity = {kind:'app', key:'test', label:'AnExtremelyLongApplicationNameWithoutAnySpacesForLayoutChecks', focused_seconds:123456, visits:800, median_visit_seconds:1234, days_used:17}
      var domain = {kind:'domain', key:'very-long-domain-name-for-layout-checks.example.com', label:'very-long-domain-name-for-layout-checks.example.com', focused_seconds:54321, visits:80, median_visit_seconds:1234, days_used:7}
      panel.activityAnalytics = {activities:empty ? [] : [activity, domain]}
      panel.activityDetail = c.state === 'selected' ? {activities:[domain],daily:panel.daily,heatmap:panel.heatmap,insights:[]} : null
      panel.totalFocused = empty ? 0 : 123456
      panel.totalMultitasked = empty ? 0 : 3200
      panel.reportInsights = empty || c.state === 'selected' ? [] : [{kind:'audio-companion', title:'Music accompanies focused work', value:'53m', explanation:'Background audio played during focused activity in the recorded period.', supporting:{}}]
      panel.inspectedInsight = c.state === 'evidence' ? panel.reportInsights[0] : null
      var start = new Date(2026,8,21).getTime()/1000
      panel.multitasking = {timeline:{start:start,end:start+86400,segments:empty ? [] : [
        {start:start+3600,end:start+5400,app_class:'test',label:activity.label,audio:[]},
        {start:start+6000,end:start+9600,app_class:'browser',label:'Browser',audio:[{app_class:'music',label:'Music',attribution:'app'}]}
      ]}}
      panel.auditScroll.contentY = 0
      timer.restart()
    }
    function inspect(item, failures) {
      if (!item.visible || item.opacity === 0) return
      if (item.text && item.contentWidth !== undefined && item.width > 0 && item.contentWidth > item.width + 2 && (!item.elide || item.elide === 3))
        failures.push({text:item.text, width:item.width, contentWidth:item.contentWidth})
      if (item.objectName === 'appearanceSettings') {
        var ancestor = item.parent
        while (ancestor && ancestor !== panel.auditBody) ancestor = ancestor.parent
        if (!ancestor) failures.push({settingsOutsideScroller:true})
      }
      if (typeof item.clicked === 'function' && item.width > 0) {
        var point = item.mapToItem(panel.auditKeys, 0, 0)
        if (point.x < -2 || point.x + item.width > panel.auditKeys.width + 2)
          failures.push({control:item.text || item.objectName, x:point.x, width:item.width, available:panel.auditKeys.width})
      }
      for (var child of item.children || []) inspect(child, failures)
    }
    Timer {
      id: timer
      interval: 100
      onTriggered: {
        var failures = []
        win.inspect(panel, failures)
        if (panel.auditScroll.height < 80) failures.push({viewportHeight:panel.auditScroll.height})
        if (panel.auditBody.width > panel.auditScroll.width) failures.push({bodyWidth:panel.auditBody.width})
        if (panel.auditSupport.visible && !Array.from(panel.auditSupport.children).some(function(item) { return item.visible }))
          failures.push({emptySupportRail:true})
        console.log('AUDIT ' + JSON.stringify({test:win.cases[win.step], failures:failures}))
        // Save all populated layouts and compact settings; assertions cover every state.
        var c = win.cases[win.step]
        if (c.state === 'populated' || c.state === 'settings' && c.width === 380) {
          var name = c.width + '-' + c.lens + '-' + c.mode + '-' + c.state
          panel.auditBody.grabToImage(function(b) { b.saveToFile(win.output + '/body-' + name + '.png') })
          win.contentItem.grabToImage(function(r) { r.saveToFile(win.output + '/' + name + '.png'); win.advance() })
        } else win.advance()
      }
    }
  }
}
