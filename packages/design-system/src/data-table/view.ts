// A DataTable's view (sort order and hidden columns) as plain data, so a route can keep it in the
// URL: `?sort=-date&hide=phone,address`. Routes validate the strings in `validateSearch` and pass
// the view to the table (client-application.md § State ownership: URL state in the router).

export interface DataTableSort {
  id: string
  desc: boolean
}

export interface DataTableView {
  /** At most one column; empty when unsorted. */
  sort: DataTableSort[]
  /** Ids of hidden columns. */
  hidden: string[]
}

/** The view's search parameters; an absent key means the default. */
export interface DataTableSearch {
  sort?: string | undefined
  hide?: string | undefined
}

export const emptyView: DataTableView = { sort: [], hidden: [] }

/** `{ sort: [{ id: 'date', desc: true }], hidden: ['phone'] }` → `{ sort: '-date', hide: 'phone' }`. */
export function viewToSearch(view: DataTableView): DataTableSearch {
  const search: DataTableSearch = {}
  const sort = view.sort.map((s) => (s.desc ? `-${s.id}` : s.id)).join(',')
  if (sort !== '') search.sort = sort
  if (view.hidden.length > 0) search.hide = view.hidden.join(',')
  return search
}

/**
 * The view from search parameters, keeping only ids in `columnIds`: a stale or edited URL never
 * reaches the table. `sortable` limits sorting to the columns that allow it.
 */
export function viewFromSearch(search: DataTableSearch, columnIds: readonly string[], sortable: readonly string[] = columnIds): DataTableView {
  const known = new Set(columnIds)
  const canSort = new Set(sortable)
  const sort = (search.sort ?? '')
    .split(',')
    .filter((part) => part !== '')
    .map((part) => (part.startsWith('-') ? { id: part.slice(1), desc: true } : { id: part, desc: false }))
    .filter((s) => canSort.has(s.id))
    .slice(0, 1)
  const hidden = [...new Set((search.hide ?? '').split(',').filter((id) => known.has(id)))]
  // Every column hidden would leave an empty grid; show them all instead.
  return { sort, hidden: hidden.length >= columnIds.length ? [] : hidden }
}
