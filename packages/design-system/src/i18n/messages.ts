import { defineMessages } from 'react-intl'

type DateView = 'day' | 'month' | 'year'

/** Each message's ICU arguments, so `formatMessage` checks the values passed. */
type Values = {
  dialogClose: {}
  toastClose: {}
  toastRegion: {}
  selectClear: {}
  comboboxTrigger: {}
  comboboxClear: {}
  comboboxEmpty: {}
  datePickerOpen: { open: 'true' | 'false' }
  datePickerClear: {}
  datePickerCalendar: {}
  datePickerPrevious: { view: DateView }
  datePickerNext: { view: DateView }
  datePickerView: { view: DateView; range: string }
}

// Every string the design system shows or announces. Ids are `ds.<component>.<name>`; apps
// translate them in their catalogs alongside their own (client-application.md § Design system).
export const messages = defineMessages<Values>({
  dialogClose: { id: 'ds.dialog.close', defaultMessage: 'Close', description: 'Accessible name of the button that closes a dialog' },
  toastClose: { id: 'ds.toast.close', defaultMessage: 'Dismiss notification', description: 'Accessible name of the button that dismisses a notification' },
  toastRegion: { id: 'ds.toast.region', defaultMessage: 'Notifications', description: 'Accessible name of the region that holds notifications' },
  selectClear: { id: 'ds.select.clear', defaultMessage: 'Clear selection', description: 'Accessible name of the button that clears a select' },
  comboboxTrigger: { id: 'ds.combobox.trigger', defaultMessage: 'Show options', description: 'Accessible name of the button that opens a combobox list' },
  comboboxClear: { id: 'ds.combobox.clear', defaultMessage: 'Clear value', description: 'Accessible name of the button that clears a combobox' },
  comboboxEmpty: { id: 'ds.combobox.empty', defaultMessage: 'No matches', description: 'Shown when no combobox option matches the typed text' },
  datePickerOpen: { id: 'ds.datePicker.open', defaultMessage: '{open, select, true {Close calendar} other {Open calendar}}', description: 'Accessible name of the calendar button' },
  datePickerClear: { id: 'ds.datePicker.clear', defaultMessage: 'Clear date', description: 'Accessible name of the button that clears a date' },
  datePickerCalendar: { id: 'ds.datePicker.calendar', defaultMessage: 'Calendar', description: 'Accessible name of the calendar popup' },
  datePickerPrevious: {
    id: 'ds.datePicker.previous',
    defaultMessage: '{view, select, day {Previous month} month {Previous year} other {Previous decade}}',
    description: 'Accessible name of the button that pages the calendar back',
  },
  datePickerNext: {
    id: 'ds.datePicker.next',
    defaultMessage: '{view, select, day {Next month} month {Next year} other {Next decade}}',
    description: 'Accessible name of the button that pages the calendar forward',
  },
  datePickerView: {
    id: 'ds.datePicker.view',
    defaultMessage: '{range}, {view, select, day {choose a month} month {choose a year} other {choose a decade}}',
    description: 'Accessible name of the calendar heading button that zooms out; range is its visible text, such as October 2026',
  },
})

/** The English catalog, `id → ICU message`: the source translators work from. */
export const englishMessages: Record<string, string> = Object.fromEntries(
  Object.values(messages).map((m) => [m.id, m.defaultMessage as string]),
)
