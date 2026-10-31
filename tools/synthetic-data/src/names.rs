//! Name and place data. The lists are short and illustrative; generated tails fill each pool to
//! the spike's size, so frequent names repeat the way real registers do (patient-identity research,
//! § Matching-scale spike).

use crate::rng::{Rng, Weighted};

/// A region of Uganda, with how its people are usually named.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Region {
    /// Buganda and its neighbours: a surname comes from the father's clan, so siblings share a
    /// clan but usually not a surname.
    Central,
    /// Banyankole, Bakiga and Batooro: theophoric names, often not shared in a household.
    Western,
    /// Basoga, Bagisu and Iteso.
    Eastern,
    /// Acholi, Langi and West Nile: Luo names take `O-` for males and `A-` for females.
    Northern,
}

impl Region {
    /// Every region, with its share of the population.
    pub const ALL: [(Self, f64); 4] = [
        (Self::Central, 0.27),
        (Self::Western, 0.25),
        (Self::Eastern, 0.26),
        (Self::Northern, 0.22),
    ];

    /// The region's display name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Central => "Central",
            Self::Western => "Western",
            Self::Eastern => "Eastern",
            Self::Northern => "Northern",
        }
    }

    /// The region's districts, most populous first (a subset of the 146).
    pub fn districts(self) -> &'static [&'static str] {
        match self {
            Self::Central => &[
                "Wakiso",
                "Kampala",
                "Mukono",
                "Masaka",
                "Luwero",
                "Mpigi",
                "Mubende",
                "Mityana",
                "Kayunga",
                "Rakai",
                "Nakaseke",
                "Kalangala",
            ],
            Self::Western => &[
                "Mbarara",
                "Kasese",
                "Kabale",
                "Hoima",
                "Bushenyi",
                "Kabarole",
                "Masindi",
                "Ntungamo",
                "Kisoro",
                "Ibanda",
                "Rukungiri",
            ],
            Self::Eastern => &[
                "Jinja",
                "Mbale",
                "Iganga",
                "Tororo",
                "Soroti",
                "Kamuli",
                "Busia",
                "Kumi",
                "Pallisa",
                "Kapchorwa",
                "Bugiri",
            ],
            Self::Northern => &[
                "Gulu", "Lira", "Arua", "Kitgum", "Apac", "Moroto", "Adjumani", "Pader", "Nebbi",
                "Kotido", "Yumbe",
            ],
        }
    }

    /// Frequent surnames: (male, female). Equal halves mean the name is not gendered.
    fn surnames(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Self::Central => &[
                ("Mukasa", "Namukasa"),
                ("Kato", "Nakato"),
                ("Ssemakula", "Nakimuli"),
                ("Lubega", "Nalubega"),
                ("Nsubuga", "Nansubuga"),
                ("Kiggundu", "Nantongo"),
                ("Musoke", "Nabukenya"),
                ("Lwanga", "Nalwanga"),
                ("Kasozi", "Nakasozi"),
                ("Mutebi", "Namutebi"),
                ("Mukiibi", "Nakiibi"),
                ("Kiwanuka", "Nakiwanuka"),
                ("Matovu", "Namatovu"),
                ("Kaggwa", "Nankya"),
                ("Bukenya", "Nabukeera"),
                ("Ssali", "Nassali"),
                ("Kalema", "Nakalema"),
                ("Mayanja", "Namayanja"),
                ("Kibirige", "Nakibirige"),
                ("Ssekandi", "Nalwoga"),
                ("Walusimbi", "Namubiru"),
                ("Kakooza", "Nakigudde"),
                ("Ssempijja", "Nampijja"),
                ("Lule", "Nalule"),
            ],
            Self::Western => &[
                ("Mugisha", "Kyomugisha"),
                ("Tumusiime", "Tumusiime"),
                ("Byaruhanga", "Ainembabazi"),
                ("Tumwesigye", "Atuhaire"),
                ("Ahimbisibwe", "Ahimbisibwe"),
                ("Twinomujuni", "Natukunda"),
                ("Turyahikayo", "Akankwasa"),
                ("Kamugisha", "Kemigisha"),
                ("Byamugisha", "Kansiime"),
                ("Asiimwe", "Asiimwe"),
                ("Arinaitwe", "Ninsiima"),
                ("Nuwagaba", "Ankunda"),
                ("Mwesigwa", "Ayebare"),
                ("Musinguzi", "Orishaba"),
                ("Kabagambe", "Tusiime"),
                ("Bagonza", "Kobusingye"),
            ],
            Self::Eastern => &[
                ("Waiswa", "Babirye"),
                ("Kakaire", "Naigaga"),
                ("Isabirye", "Nabirye"),
                ("Mugoya", "Namono"),
                ("Kirunda", "Nambozo"),
                ("Wandera", "Nekesa"),
                ("Wanyama", "Nafula"),
                ("Wafula", "Nasimiyu"),
                ("Masaba", "Namataka"),
                ("Mafabi", "Nabwire"),
                ("Wabwire", "Nakayenze"),
                ("Okiror", "Amongin"),
                ("Okurut", "Asio"),
                ("Ekwaro", "Atai"),
                ("Emuria", "Akello"),
                ("Balikowa", "Kagoya"),
            ],
            Self::Northern => &[
                ("Okello", "Akello"),
                ("Ocen", "Acen"),
                ("Opio", "Apio"),
                ("Ojok", "Ajok"),
                ("Odongo", "Adong"),
                ("Okot", "Akot"),
                ("Ocan", "Acan"),
                ("Otim", "Atim"),
                ("Okwir", "Akwir"),
                ("Ogwang", "Agwang"),
                ("Onen", "Anena"),
                ("Oryema", "Aryemo"),
                ("Olanya", "Alanyo"),
                ("Drani", "Ayikoru"),
                ("Anguyo", "Candiru"),
                ("Bayo", "Driciru"),
            ],
        }
    }

    /// Syllables for the generated tail of the region's pool.
    fn syllables(self) -> (&'static [&'static str], &'static [&'static str]) {
        match self {
            Self::Central => (
                &[
                    "Ka", "Ki", "Mu", "Ssa", "Lu", "Nsu", "Kya", "Bu", "Wa", "Se",
                ],
                &[
                    "ba", "ga", "la", "ma", "ndi", "nga", "nyo", "ko", "zi", "tte", "bi", "wo",
                ],
            ),
            Self::Western => (
                &[
                    "Tu", "Byo", "Ka", "Mwe", "Bya", "Twi", "Ru", "Ki", "Ny", "Bi",
                ],
                &[
                    "ru", "ha", "nga", "si", "me", "mbo", "gi", "sha", "ra", "bwa", "ji", "ko",
                ],
            ),
            Self::Eastern => (
                &["Wa", "Ma", "Ki", "Mu", "Ba", "Wo", "Bu", "Ise", "Lu", "Ka"],
                &[
                    "fu", "nya", "ndo", "si", "be", "ra", "kwe", "le", "nde", "mbe", "yi", "la",
                ],
            ),
            Self::Northern => (
                &["Ok", "Od", "Ob", "Ol", "Om", "Oc", "Ow", "Op", "Ot", "Og"],
                &[
                    "ello", "ongo", "wor", "anya", "iti", "uru", "ala", "ema", "aro", "eng",
                ],
            ),
        }
    }
}

/// A region's surname pool, frequent names first, with a Zipf–Mandelbrot frequency.
#[derive(Debug, Clone)]
pub struct SurnamePool {
    region: Region,
    names: Vec<(String, String)>,
    frequency: Weighted,
}

/// Names per regional pool, as in the matching-scale spike.
pub const POOL_SIZE: usize = 1_250;

impl SurnamePool {
    /// The pool for `region`: its frequent names, then generated ones to [`POOL_SIZE`].
    pub fn new(region: Region, seed: u64) -> Self {
        let mut names: Vec<(String, String)> = region
            .surnames()
            .iter()
            .map(|&(m, f)| (m.to_owned(), f.to_owned()))
            .collect();
        let (heads, tails) = region.syllables();
        let mut rng = Rng::new(seed, 0x5052_4e00 + region as u64);
        while names.len() < POOL_SIZE {
            let mut name = (*rng.pick(heads)).to_owned();
            // Up to three syllables: every region then has well over POOL_SIZE combinations.
            for _ in 0..rng.between(1, 3) {
                name.push_str(rng.pick(tails));
            }
            if names.iter().any(|(m, _)| *m == name) {
                continue;
            }
            let female = match region {
                // Luo names change the leading vowel; Ganda female names often take `Na-`.
                Region::Northern => format!("A{}", &name[1..]),
                Region::Central => format!("Na{}", name.to_lowercase()),
                Region::Western | Region::Eastern => name.clone(),
            };
            names.push((name, female));
        }
        Self {
            region,
            frequency: Weighted::zipf(names.len(), 1.05, 2.7),
            names,
        }
    }

    /// The region this pool serves.
    pub fn region(&self) -> Region {
        self.region
    }

    /// A pool rank, by frequency.
    pub fn rank(&self, rng: &mut Rng) -> usize {
        self.frequency.sample(rng)
    }

    /// The surname at `rank` for a person of `sex`.
    pub fn name(&self, rank: usize, female: bool) -> &str {
        let (male, woman) = &self.names[rank % self.names.len()];
        if female { woman } else { male }
    }

    /// How many names the pool holds.
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// Whether the pool is empty (never, once built).
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}

/// Given names: English or Christian, then Muslim (about 14% of Ugandans).
pub const GIVEN_MALE: &[&str] = &[
    "Joseph", "John", "Moses", "Peter", "Paul", "Samuel", "David", "Daniel", "Isaac", "Robert",
    "Emmanuel", "Ronald", "Godfrey", "Fred", "Charles", "Brian", "Ivan", "Allan", "Denis",
    "Patrick", "Richard", "Henry", "Simon", "Geoffrey", "Andrew", "Julius", "Stephen", "James",
];
/// Female given names, in the same order of origin.
pub const GIVEN_FEMALE: &[&str] = &[
    "Grace",
    "Sarah",
    "Esther",
    "Ruth",
    "Mary",
    "Patience",
    "Prossy",
    "Brenda",
    "Agnes",
    "Rose",
    "Joan",
    "Juliet",
    "Florence",
    "Harriet",
    "Annet",
    "Doreen",
    "Gloria",
    "Sharon",
    "Rebecca",
    "Christine",
    "Scovia",
    "Immaculate",
    "Winnie",
    "Betty",
    "Jane",
    "Lydia",
    "Margaret",
    "Irene",
];
/// Muslim given names.
pub const GIVEN_MALE_MUSLIM: &[&str] = &[
    "Hassan", "Ali", "Musa", "Ibrahim", "Yusuf", "Swaibu", "Hamza", "Abdul",
];
/// Muslim given names for women.
pub const GIVEN_FEMALE_MUSLIM: &[&str] = &[
    "Hadija", "Aisha", "Zainab", "Mariam", "Fatuma", "Shamim", "Sumaya", "Rehema",
];

/// Ganda twin names, by birth order: (male, female).
pub const GANDA_TWINS: [(&str, &str); 2] = [("Waswa", "Babirye"), ("Kato", "Nakato")];

/// A generated village or parish name.
pub fn village(rng: &mut Rng) -> String {
    const HEADS: &[&str] = &["Ki", "Bu", "Ka", "Nya", "Lu", "Mu", "Pa", "Ra", "Ki", "Na"];
    const TAILS: &[&str] = &[
        "sasi", "kedi", "bale", "mbogo", "gala", "wero", "lenga", "sozi", "tuku", "manya",
    ];
    format!("{}{}", rng.pick(HEADS), rng.pick(TAILS))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pools_are_full_and_distinct() {
        for (region, _) in Region::ALL {
            let pool = SurnamePool::new(region, 1);
            assert_eq!(pool.len(), POOL_SIZE);
            let mut males: Vec<&str> = (0..pool.len()).map(|i| pool.name(i, false)).collect();
            males.sort_unstable();
            males.dedup();
            assert_eq!(males.len(), POOL_SIZE, "{region:?}");
        }
    }

    #[test]
    fn luo_names_are_gendered_by_their_first_vowel() {
        let pool = SurnamePool::new(Region::Northern, 1);
        for rank in 0..pool.len() {
            let (male, female) = (pool.name(rank, false), pool.name(rank, true));
            if male.starts_with('O') {
                assert!(female.starts_with('A'), "{male} / {female}");
            }
        }
    }
}
