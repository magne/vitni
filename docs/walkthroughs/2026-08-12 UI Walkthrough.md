# Issues found during UI walkthrough

Performed: 2026-08-12.

## Mockups

### Design system

- There is a (probbly) drop-down list displayed in the top left corner (contains 'Berg, Anna', 'Lovelace, Anna', and
  '+ New person "ann"...'). Don't know where it really belongs.

## Dashboard

### Recent Activity Pane

- A collection history node (usually from an import) should be expandable/retractable. If possible, I think it should
  expand to a slightly indented list of collapsed record nodes, which again expands to a slightly more indented list
  of history nodes for that record.
- A collection history node (both levels) should show a (muted) count of contained nodes.

### Other

- The '+' new record "tab" should use record singluar, not plural (e.g. _New Person_, not _New People_).
- Similarly, the tab label for a new record should use the singular.
- Closing a non-dirty new record should not present the _Close tab?_ dialog, but just close it.
- TODO The new record keystroke (⌘N) does not always work, and will not always show the record selection dialog.

## Shared Record Tabs

### General

- In the tab header, the _Confidential_, _Locked_, and _Privacy_ buttons is a bit hard to distinguish from the badges
  to the left of them. Maybe we could have a bit more padding between the badges and the buttons.
- Should we make the above buttons into a toggle group (given that we can retain the current 'on' display colors etc.)?
- There is no way to add a reason for the change to any of the above buttons. Maybe toggeling a button should switch
  the entire record into edit mode? Or should we use the model that Tags use (_Restrictions_ section, and only toggle
  when in edit mode. But I still want the display in the header, not as a Restrictions section in the read-only record)?
- In any tab, I think an explanation of the tab (e.g. as on the _Research Note_ and _History_ tab) should be at the top.
- Below the (optional) tab explanation should come any _Add_ or _Attach_ button. I think I would prefer this button
  to the right, not to the left as it is now. If there are multiple actions, can we create a button that executes the
  labelled action if pressed on the label, but has a drop-down to the right (with a | between them) that will show a
  drop down of actions. If an action is selected, it is executed. Like the _New email_ button in Outlook web.
- Some tabs have an _Add_ button (Research Note), some have an _Attach_ button (_Add tag_, _Attach note_, _Attach
  media_, ...). Do you think we should always be able to both _add_ (create a new record) and _attach_ (use an
  existing record)? If so, should this be two separate actions on the button, or should we use the search field (e.g.
  _Find citation..._) to add a new record (_+ New citation_).
- When adding/attaching anything to a record, the _Reason for this change_ text field in the dialog will delete
  anything typed as soon as it is typed.
- Add/Attach buttons should have a '+' icon to the left of the label.
- _Nothing here yet_ or attached records should also be below the explanation, left of the button.
- All elements (explanation, button, records) should be properly padded from the sides and each other.
- We should have a coherent display of referenced records (e.g. _Notes_ is very basic, _Research Notes_ is a table,
  Person _Citations_ is maybe what we should aim for).
- In tab tables, action buttons (probably Ghost) are not really distinguishable as a button when the table row is highlighted.

### Citations Tab

- Is this tab shared? Should it be? In the _Persons_ citation tab, the _Detach_ button (probably Ghost) is not
  really distinguishable as a button when the table row is highlighted.
- _Evidence_ is never displayed.

### Media Tab

- The media element/preview never shows the image. Same when a media element is clicked.
- Instead of the _Detatch_ button, maybe we should have a _(x)_ icon in the top right corner?

### Notes Tab

- Add an explanation, differentiate from _Research Note_.
- Should display like in mockups (e.g. see media mockup)

### Tags Tab

- Each tag should be a bit larger.

### History Tab

- A collection history node (usually from an import) should be expandable/retractable, showing each history node
  slightly indented.
- The comment for a collection history node will show for example _5 records imported_, which is correct on the
  dashboard _Recent Activity_ pane, but not for the history tab for a particular record. A better comment here
  would be for example _Imported from Digitalarkivet_.
- A collection history node should show a (muted) count of contained nodes.

## Media Record

- Preview doesn't display image.
- File section:
  - Don't display ID (it's in header).
  - Field label and value should be on same line.
- Used by:
  - Make sure colums line up.
- Edit mode:
  - _File path_ should indicate if file found, but not prevent saving if not found.
  - If _Web path_ changed when saving and _File path_ set, offer to download. Ask if overwrite if exists and
    different (use checksum).
  - Try to determine mime type from file or web if not set.

## Tag Record

- In the header, the color badge does not show the color dot left of the color value.
- I want the read-only overview like in the mockup. Field label and value on the same line. Same when in edit mode,
  field label and input on same line, but still spilt in _Tag_ and _Color_ sections, not as in mockup. Update mockup
  for edit mode.
- The Restrictions section should be changed according to the decision made in the General section of this document.
- The curren edit-mode swatch is OK, but the read-only swatch should be like the mockup.
- The read-only color section misses the preview.
