import type { ComponentProps, ComponentType } from 'react'

import { cx } from '../../styled-system/css'


/** An Ark part with its recipe slot class; a `className` prop is added after it. */
// Any component; its own props are checked where the part is used.
// oxlint-disable-next-line typescript/no-explicit-any
export function slot<C extends ComponentType<any>>(Component: C, slotClass: string) {
  const Part = Component as ComponentType<{ className?: string | undefined }>
  function Slotted(props: ComponentProps<C>) {
    const { className } = props as { className?: string | undefined }
    return <Part {...(props as object)} className={cx(slotClass, className)} />
  }
  Slotted.displayName = `Slot(${slotClass})`
  return Slotted
}
