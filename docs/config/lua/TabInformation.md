# TabInformation

The `TabInformation` struct describes a tab.  `TabInformation` is purely a
snapshot of some of the key characteristics of the tab, intended for use in
synchronous, fast, event callbacks that format GUI elements such as the window
and tab title bars.

The `TabInformation` struct contains the following fields:

* `tab_id` - the identifier for the tab
* `tab_index` - the logical tab position within its containing window, with 0 indicating the leftmost tab
* `is_active` - is true if this tab is the active tab
* `is_last_active` - is true if this tab is the previously active tab. {{since('nightly', inline=True)}}
* `broadcast_input` - is true if [ToggleBroadcastInput](keyassignment/ToggleBroadcastInput.md) is on for this tab. {{since('nightly', inline=True)}}
* `has_unseen_bell` - is true if a bell rang in this tab while it wasn't the focused tab, and you haven't looked at it since. See [bell_on_tab](config/bell_on_tab.md). {{since('nightly', inline=True)}}
* `active_pane` - the [PaneInformation](PaneInformation.md) for the active pane in this tab
* `panes` - the [PaneInformation](PaneInformation.md) for all panes in this tab {{since('20220319-142410-0fcdea07', inline=True)}}
* `window_id` - the ID of the window that contains this tab {{since('20220807-113146-c2fee766', inline=True)}}
* `window_title` - the title of the window that contains this tab {{since('20220807-113146-c2fee766', inline=True)}}
* `tab_title` - the title of the tab {{since('20220807-113146-c2fee766', inline=True)}}


