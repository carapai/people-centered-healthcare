import './index.css'

import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'

import { Gallery } from './gallery'

const root = document.getElementById('root')
if (root === null) {
  throw new Error('index.html must contain #root')
}
createRoot(root).render(
  <StrictMode>
    <Gallery />
  </StrictMode>,
)
