import { DatePicker as ArkDatePicker, type DatePickerRootProps } from '@ark-ui/react/date-picker'
import { CalendarDays, ChevronLeft, ChevronRight, X } from 'lucide-react'
import { useIntl } from 'react-intl'

import { datePicker } from '../../styled-system/recipes'
import { messages } from '../i18n/messages'
import { slot } from './slot'

const classes = datePicker()

const Root = slot(ArkDatePicker.Root, classes.root)

function LocalizedRoot(props: DatePickerRootProps) {
  const intl = useIntl()
  return (
    <Root
      translations={{
        trigger: (open) => intl.formatMessage(messages.datePickerOpen, { open: open ? 'true' : 'false' }),
        clearTrigger: intl.formatMessage(messages.datePickerClear),
        content: intl.formatMessage(messages.datePickerCalendar),
        prevTrigger: (view) => intl.formatMessage(messages.datePickerPrevious, { view }),
        nextTrigger: (view) => intl.formatMessage(messages.datePickerNext, { view }),
      }}
      {...props}
    />
  )
}

const Trigger = slot(ArkDatePicker.Trigger, classes.trigger)
const ClearTrigger = slot(ArkDatePicker.ClearTrigger, classes.clearTrigger)
const View = slot(ArkDatePicker.View, classes.view)
const ViewControl = slot(ArkDatePicker.ViewControl, classes.viewControl)
const PrevTrigger = slot(ArkDatePicker.PrevTrigger, classes.prevTrigger)
const NextTrigger = slot(ArkDatePicker.NextTrigger, classes.nextTrigger)
const ViewTrigger = slot(ArkDatePicker.ViewTrigger, classes.viewTrigger)
const RangeText = slot(ArkDatePicker.RangeText, classes.rangeText)
const Table = slot(ArkDatePicker.Table, classes.table)
const TableHead = slot(ArkDatePicker.TableHead, classes.tableHead)
const TableBody = slot(ArkDatePicker.TableBody, classes.tableBody)
const TableRow = slot(ArkDatePicker.TableRow, classes.tableRow)
const TableHeader = slot(ArkDatePicker.TableHeader, classes.tableHeader)
const TableCell = slot(ArkDatePicker.TableCell, classes.tableCell)
const TableCellTrigger = slot(ArkDatePicker.TableCellTrigger, classes.tableCellTrigger)

function ViewHeader() {
  const intl = useIntl()
  return (
    <ViewControl>
      <PrevTrigger>
        <ChevronLeft size={18} aria-hidden />
      </PrevTrigger>
      <ArkDatePicker.Context>
        {(api) => (
          // The accessible name starts with the visible range (WCAG 2.5.3 label in name).
          <ViewTrigger aria-label={intl.formatMessage(messages.datePickerView, { view: api.view, range: api.visibleRangeText.start })}>
            <RangeText />
          </ViewTrigger>
        )}
      </ArkDatePicker.Context>
      <NextTrigger>
        <ChevronRight size={18} aria-hidden />
      </NextTrigger>
    </ViewControl>
  )
}

/** The day, month and year views of the calendar. */
function Calendar() {
  return (
    <>
      <View view="day">
        <ArkDatePicker.Context>
          {(api) => (
            <>
              <ViewHeader />
              <Table>
                <TableHead>
                  <TableRow>
                    {api.weekDays.map((day, i) => (
                      <TableHeader key={i} abbr={day.long}>
                        {day.narrow}
                      </TableHeader>
                    ))}
                  </TableRow>
                </TableHead>
                <TableBody>
                  {api.weeks.map((week, i) => (
                    <TableRow key={i}>
                      {week.map((day, j) => (
                        <TableCell key={j} value={day}>
                          <TableCellTrigger>{day.day}</TableCellTrigger>
                        </TableCell>
                      ))}
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </>
          )}
        </ArkDatePicker.Context>
      </View>
      <View view="month">
        <ArkDatePicker.Context>
          {(api) => (
            <>
              <ViewHeader />
              <Table>
                <TableBody>
                  {api.getMonthsGrid({ columns: 4, format: 'short' }).map((months, i) => (
                    <TableRow key={i}>
                      {months.map((month, j) => (
                        <TableCell key={j} value={month.value}>
                          <TableCellTrigger>{month.label}</TableCellTrigger>
                        </TableCell>
                      ))}
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </>
          )}
        </ArkDatePicker.Context>
      </View>
      <View view="year">
        <ArkDatePicker.Context>
          {(api) => (
            <>
              <ViewHeader />
              <Table>
                <TableBody>
                  {api.getYearsGrid({ columns: 4 }).map((years, i) => (
                    <TableRow key={i}>
                      {years.map((year, j) => (
                        <TableCell key={j} value={year.value}>
                          <TableCellTrigger>{year.label}</TableCellTrigger>
                        </TableCell>
                      ))}
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </>
          )}
        </ArkDatePicker.Context>
      </View>
    </>
  )
}

export const DatePicker = {
  Root: LocalizedRoot,
  Label: slot(ArkDatePicker.Label, classes.label),
  Control: slot(ArkDatePicker.Control, classes.control),
  Input: slot(ArkDatePicker.Input, classes.input),
  Trigger: () => (
    <Trigger>
      <CalendarDays size={18} aria-hidden />
    </Trigger>
  ),
  ClearTrigger: () => (
    <ClearTrigger>
      <X size={18} aria-hidden />
    </ClearTrigger>
  ),
  Positioner: slot(ArkDatePicker.Positioner, classes.positioner),
  Content: slot(ArkDatePicker.Content, classes.content),
  Calendar,
}
