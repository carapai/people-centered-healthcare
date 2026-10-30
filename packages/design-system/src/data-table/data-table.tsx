import { Menu as ArkMenu } from '@ark-ui/react/menu'
import {
  columnVisibilityFeature,
  createSortedRowModel,
  rowSortingFeature,
  tableFeatures,
  useTable,
  type ColumnDef,
  type ColumnVisibilityState,
  type RowData,
  type SortFn,
  type SortingState,
  type Updater,
} from '@tanstack/react-table'
import { defaultRangeExtractor, useVirtualizer } from '@tanstack/react-virtual'
import { ArrowDown, ArrowUp, ArrowUpDown, Check, Columns3 } from 'lucide-react'
import { useId, useLayoutEffect, useMemo, useRef, useState, type CSSProperties, type KeyboardEvent, type ReactNode } from 'react'
import { useIntl } from 'react-intl'

import { cx } from '../../styled-system/css'
import { dataTable, menu } from '../../styled-system/recipes'
import { messages } from '../i18n/messages'
import type { DataTableView } from './view'

// The DataTable (client-application.md § Design system): TanStack Table 9 for sorting and column
// visibility, TanStack Virtual 3 for the rows, and the ARIA grid pattern
// (https://www.w3.org/WAI/ARIA/apg/patterns/grid/) for semantics and the keyboard model. Only the
// rows in view, plus the focused one, are in the DOM.

export interface DataTableColumn<T> {
  id: string
  /** The column header, already localized. */
  header: string
  /** The cell's content. */
  cell: (row: T) => ReactNode
  /** The value the column sorts by; without it the column does not sort. */
  sortValue?: ((row: T) => string | number | undefined) | undefined
  /** A CSS grid track; default `minmax(8rem, 1fr)`. */
  width?: string | undefined
  /** Whether the user may hide the column; default true. */
  canHide?: boolean | undefined
  /** `end` for numbers. */
  align?: 'start' | 'end' | undefined
}

export interface DataTableProps<T extends RowData> {
  /** The table's visible title, which also names the grid. */
  caption: string
  columns: readonly DataTableColumn<T>[]
  data: readonly T[]
  getRowId: (row: T) => string
  /** Sort and hidden columns; the caller owns it, usually in the URL (see `view.ts`). */
  view: DataTableView
  onViewChange: (view: DataTableView) => void
  /** Enter on a body cell. */
  onRowActivate?: ((row: T) => void) | undefined
  /** The scrolling grid's height; default `28rem`. */
  height?: string | undefined
}

const features = tableFeatures({ rowSortingFeature, columnVisibilityFeature, sortedRowModel: createSortedRowModel() })

const classes = dataTable()
const menuClasses = menu()

interface Position {
  /** 0 is the header row; 1… are body rows. */
  row: number
  col: number
}

function resolve<S>(updater: Updater<S>, old: S): S {
  return typeof updater === 'function' ? (updater as (old: S) => S)(old) : updater
}

function hiddenOf(visibility: ColumnVisibilityState): string[] {
  return Object.entries(visibility)
    .filter(([, visible]) => !visible)
    .map(([id]) => id)
}

export function DataTable<T extends RowData>({ caption, columns, data, getRowId, view, onViewChange, onRowActivate, height = '28rem' }: DataTableProps<T>) {
  const intl = useIntl()
  const captionId = useId()
  const gridRef = useRef<HTMLDivElement>(null)
  const headRef = useRef<HTMLDivElement>(null)
  const [announcement, setAnnouncement] = useState('')

  const byId = useMemo(() => new Map(columns.map((c) => [c.id, c])), [columns])
  const columnDefs = useMemo(() => {
    const collator = new Intl.Collator(intl.locale, { numeric: true, sensitivity: 'base' })
    const compare: SortFn<typeof features, T> = (a, b, id) => {
      const x = a.getValue<string | number>(id)
      const y = b.getValue<string | number>(id)
      return typeof x === 'number' && typeof y === 'number' ? x - y : collator.compare(String(x), String(y))
    }
    return columns.map(
      (c): ColumnDef<typeof features, T> => ({
        id: c.id,
        header: c.header,
        accessorFn: (row: T) => c.sortValue?.(row),
        enableSorting: c.sortValue !== undefined,
        enableHiding: c.canHide ?? true,
        sortFn: compare,
        sortUndefined: 'last',
        // Every column sorts ascending first, numbers included.
        sortDescFirst: false,
      }),
    )
  }, [columns, intl.locale])

  const sorting: SortingState = view.sort
  const columnVisibility: ColumnVisibilityState = useMemo(() => Object.fromEntries(view.hidden.map((id) => [id, false])), [view.hidden])

  const table = useTable({
    features,
    columns: columnDefs,
    data: data as T[],
    getRowId: (row: T) => getRowId(row),
    state: { sorting, columnVisibility },
    enableMultiSort: false,
    enableSortingRemoval: true,
    onSortingChange: (updater) => {
      const sort = resolve(updater, sorting).slice(0, 1)
      const first = sort[0]
      const column = first === undefined ? undefined : byId.get(first.id)
      setAnnouncement(
        intl.formatMessage(messages.dataTableSorted, {
          column: column?.header ?? '',
          direction: first === undefined ? 'none' : first.desc ? 'desc' : 'asc',
        }),
      )
      onViewChange({ ...view, sort })
    },
    onColumnVisibilityChange: (updater) => onViewChange({ ...view, hidden: hiddenOf(resolve(updater, columnVisibility)) }),
  })

  const visibleColumns = table.getVisibleLeafColumns()
  const rows = table.getRowModel().rows
  const rowCount = rows.length
  const colCount = visibleColumns.length

  const [active, setActive] = useState<Position>({ row: 0, col: 0 })
  const current: Position = { row: Math.min(active.row, rowCount), col: Math.min(active.col, Math.max(colCount - 1, 0)) }
  const activeRef = useRef(current)
  activeRef.current = current
  const pendingFocus = useRef(false)

  // Row and header heights follow the density tokens, so they are measured, not assumed.
  const [geometry, setGeometry] = useState({ head: 45, row: 45 })
  const virtualizer = useVirtualizer({
    count: rowCount,
    getScrollElement: () => gridRef.current,
    estimateSize: () => geometry.row,
    getItemKey: (index) => rows[index]?.id ?? index,
    overscan: 8,
    // The focused row stays rendered when scrolled away, so Tab can always return to it.
    rangeExtractor: (range) => {
      const indexes = defaultRangeExtractor(range)
      const focused = activeRef.current.row - 1
      if (focused >= 0 && focused < range.count && !indexes.includes(focused)) {
        indexes.push(focused)
        indexes.sort((a, b) => a - b)
      }
      return indexes
    },
  })

  useLayoutEffect(() => {
    const head = headRef.current?.offsetHeight ?? geometry.head
    const firstRow = gridRef.current?.querySelector<HTMLElement>('[data-body-row]')
    const row = firstRow?.offsetHeight ?? geometry.row
    if (head !== geometry.head || row !== geometry.row) {
      setGeometry({ head, row })
      virtualizer.measure()
    }
  })

  useLayoutEffect(() => {
    if (!pendingFocus.current) return
    pendingFocus.current = false
    gridRef.current?.querySelector<HTMLElement>(`[data-cell="${current.row}:${current.col}"]`)?.focus()
  })

  function moveTo(row: number, col: number) {
    const next = { row: Math.max(0, Math.min(row, rowCount)), col: Math.max(0, Math.min(col, colCount - 1)) }
    activeRef.current = next
    setActive(next)
    pendingFocus.current = true
    if (next.row > 0) virtualizer.scrollToIndex(next.row - 1, { align: 'auto' })
  }

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    const { row, col } = current
    const page = Math.max(1, Math.floor(((gridRef.current?.clientHeight ?? 0) - geometry.head) / geometry.row))
    const ctrl = event.ctrlKey || event.metaKey
    switch (event.key) {
      case 'ArrowRight':
        moveTo(row, col + 1)
        break
      case 'ArrowLeft':
        moveTo(row, col - 1)
        break
      case 'ArrowDown':
        moveTo(row + 1, col)
        break
      case 'ArrowUp':
        moveTo(row - 1, col)
        break
      case 'PageDown':
        moveTo(row + page, col)
        break
      case 'PageUp':
        moveTo(row - page, col)
        break
      case 'Home':
        moveTo(ctrl ? 0 : row, 0)
        break
      case 'End':
        moveTo(ctrl ? rowCount : row, colCount - 1)
        break
      case 'Enter':
      case ' ': {
        if (row === 0) {
          const column = visibleColumns[col]
          if (column?.getCanSort() !== true) return
          column.toggleSorting()
        } else if (event.key === 'Enter' && onRowActivate !== undefined) {
          const target = rows[row - 1]
          if (target === undefined) return
          onRowActivate(target.original)
        } else {
          return
        }
        break
      }
      default:
        return
    }
    event.preventDefault()
  }

  function onFocus(event: { target: EventTarget }) {
    const cell = (event.target as HTMLElement).dataset.cell
    if (cell === undefined) return
    const [row, col] = cell.split(':').map(Number)
    if (row !== undefined && col !== undefined && (row !== current.row || col !== current.col)) {
      const next = { row, col }
      activeRef.current = next
      setActive(next)
    }
  }

  const tabbable = (row: number, col: number) => (row === current.row && col === current.col ? 0 : -1)
  const visibleCount = visibleColumns.length
  const gridStyle = {
    height,
    scrollPaddingTop: `${geometry.head}px`,
    '--data-table-columns': visibleColumns.map((c) => byId.get(c.id)?.width ?? 'minmax(8rem, 1fr)').join(' '),
  } as CSSProperties

  return (
    <div className={classes.root}>
      <div className={classes.toolbar}>
        <div id={captionId} className={classes.caption}>
          {caption}
        </div>
        <ArkMenu.Root closeOnSelect={false}>
          <ArkMenu.Trigger className={classes.columnsTrigger}>
            <Columns3 size={16} aria-hidden />
            {intl.formatMessage(messages.dataTableColumns)}
          </ArkMenu.Trigger>
          <ArkMenu.Positioner className={menuClasses.positioner}>
            <ArkMenu.Content className={menuClasses.content}>
              {table
                .getAllLeafColumns()
                .filter((column) => column.getCanHide())
                .map((column) => {
                  const visible = column.getIsVisible()
                  return (
                    <ArkMenu.CheckboxItem
                      key={column.id}
                      value={column.id}
                      checked={visible}
                      // The last visible column cannot be hidden.
                      disabled={visible && visibleCount === 1}
                      onCheckedChange={(checked) => column.toggleVisibility(checked)}
                      className={menuClasses.item}
                    >
                      <ArkMenu.ItemText className={menuClasses.itemText}>{byId.get(column.id)?.header}</ArkMenu.ItemText>
                      <ArkMenu.ItemIndicator className={menuClasses.itemIndicator}>
                        <Check size={16} aria-hidden />
                      </ArkMenu.ItemIndicator>
                    </ArkMenu.CheckboxItem>
                  )
                })}
            </ArkMenu.Content>
          </ArkMenu.Positioner>
        </ArkMenu.Root>
      </div>
      <div
        ref={gridRef}
        role="grid"
        aria-labelledby={captionId}
        aria-rowcount={rowCount + 1}
        aria-colcount={colCount}
        className={classes.grid}
        style={gridStyle}
        onKeyDown={onKeyDown}
        onFocus={onFocus}
      >
        <div ref={headRef} role="rowgroup" className={classes.head}>
          <div role="row" aria-rowindex={1} className={classes.headerRow}>
            {visibleColumns.map((column, col) => {
              const definition = byId.get(column.id)
              const sortable = column.getCanSort()
              const sorted = column.getIsSorted()
              return (
                <div
                  key={column.id}
                  role="columnheader"
                  aria-colindex={col + 1}
                  aria-sort={sortable ? (sorted === 'asc' ? 'ascending' : sorted === 'desc' ? 'descending' : 'none') : undefined}
                  tabIndex={tabbable(0, col)}
                  data-cell={`0:${col}`}
                  data-sortable={sortable ? '' : undefined}
                  data-align={definition?.align}
                  className={classes.header}
                  onClick={sortable ? () => column.toggleSorting() : undefined}
                >
                  {definition?.header}
                  {sortable && (
                    <span className={classes.sortIndicator} data-sorted={sorted === false ? undefined : ''}>
                      {sorted === 'asc' ? <ArrowUp size={14} aria-hidden /> : sorted === 'desc' ? <ArrowDown size={14} aria-hidden /> : <ArrowUpDown size={14} aria-hidden />}
                    </span>
                  )}
                </div>
              )
            })}
          </div>
        </div>
        <div role="rowgroup" className={classes.body} style={{ height: rowCount === 0 ? undefined : `${virtualizer.getTotalSize()}px` }}>
          {rowCount === 0 ? (
            <div role="row" aria-rowindex={2} className={cx(classes.row, classes.headerRow)} style={{ position: 'relative' }}>
              <div role="gridcell" aria-colindex={1} aria-colspan={colCount} className={classes.empty}>
                {intl.formatMessage(messages.dataTableEmpty)}
              </div>
            </div>
          ) : (
            virtualizer.getVirtualItems().map((item) => {
              const row = rows[item.index]
              if (row === undefined) return null
              const index = item.index + 1
              return (
                <div
                  key={row.id}
                  role="row"
                  aria-rowindex={index + 1}
                  data-body-row=""
                  data-active={index === current.row ? '' : undefined}
                  className={classes.row}
                  style={{ transform: `translateY(${item.start}px)` }}
                >
                  {row.getVisibleCells().map((cell, col) => {
                    const definition = byId.get(cell.column.id)
                    return (
                      <div
                        key={cell.id}
                        role="gridcell"
                        aria-colindex={col + 1}
                        tabIndex={tabbable(index, col)}
                        data-cell={`${index}:${col}`}
                        data-align={definition?.align}
                        className={classes.cell}
                      >
                        {definition?.cell(row.original)}
                      </div>
                    )
                  })}
                </div>
              )
            })
          )}
        </div>
      </div>
      <div role="status" className={classes.status}>
        {announcement}
      </div>
    </div>
  )
}
