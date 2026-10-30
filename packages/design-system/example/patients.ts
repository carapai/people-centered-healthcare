// Deterministic sample rows for the example page and the DataTable tests.

export interface Patient {
  id: string
  name: string
  sex: 'Female' | 'Male'
  age: number
  village: string
  phone: string
}

const given = ['Grace', 'Joseph', 'Sarah', 'Moses', 'Esther', 'Peter', 'Ruth', 'Isaac', 'Florence', 'David']
const family = ['Akello', 'Okello', 'Namubiru', 'Ssempala', 'Nakato', 'Mugisha', 'Atim', 'Byaruhanga', 'Nansubuga', 'Opio']
const villages = ['Kawempe', 'Nakawa', 'Rubaga', 'Makindye', 'Kira', 'Wakiso', 'Mukono', 'Entebbe']

export function patients(count: number): Patient[] {
  return Array.from({ length: count }, (_, i) => ({
    id: `p${i + 1}`,
    name: `${given[i % given.length]} ${family[Math.floor(i / given.length) % family.length]} ${i + 1}`,
    sex: i % 2 === 0 ? 'Female' : 'Male',
    age: (i * 37) % 91,
    village: villages[(i * 7) % villages.length] ?? '',
    phone: `0772 ${String(100000 + ((i * 7919) % 900000)).padStart(6, '0')}`,
  }))
}
