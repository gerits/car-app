pub struct Country {
    pub name: &'static str,
    pub geofabrik_path: &'static str,
}

pub struct Continent {
    pub name: &'static str,
    pub countries: &'static [Country],
}

pub static CONTINENTS: &[Continent] = &[
    Continent {
        name: "Europe",
        countries: &[
            Country {
                name: "Belgium",
                geofabrik_path: "europe/belgium",
            },
            Country {
                name: "Netherlands",
                geofabrik_path: "europe/netherlands",
            },
            Country {
                name: "Luxembourg",
                geofabrik_path: "europe/luxembourg",
            },
            Country {
                name: "Germany",
                geofabrik_path: "europe/germany",
            },
            Country {
                name: "France",
                geofabrik_path: "europe/france",
            },
            Country {
                name: "Great Britain",
                geofabrik_path: "europe/great-britain",
            },
            Country {
                name: "Italy",
                geofabrik_path: "europe/italy",
            },
            Country {
                name: "Spain",
                geofabrik_path: "europe/spain",
            },
            Country {
                name: "Switzerland",
                geofabrik_path: "europe/switzerland",
            },
            Country {
                name: "Austria",
                geofabrik_path: "europe/austria",
            },
            Country {
                name: "Denmark",
                geofabrik_path: "europe/denmark",
            },
            Country {
                name: "Sweden",
                geofabrik_path: "europe/sweden",
            },
            Country {
                name: "Norway",
                geofabrik_path: "europe/norway",
            },
            Country {
                name: "Finland",
                geofabrik_path: "europe/finland",
            },
            Country {
                name: "Poland",
                geofabrik_path: "europe/poland",
            },
            Country {
                name: "Portugal",
                geofabrik_path: "europe/portugal",
            },
            Country {
                name: "Ireland & N. Ireland",
                geofabrik_path: "europe/ireland-and-northern-ireland",
            },
            Country {
                name: "Monaco",
                geofabrik_path: "europe/monaco",
            },
            Country {
                name: "Andorra",
                geofabrik_path: "europe/andorra",
            },
            Country {
                name: "Liechtenstein",
                geofabrik_path: "europe/liechtenstein",
            },
        ],
    },
    Continent {
        name: "North America",
        countries: &[
            Country {
                name: "Canada",
                geofabrik_path: "north-america/canada",
            },
            Country {
                name: "Mexico",
                geofabrik_path: "north-america/mexico",
            },
            Country {
                name: "Greenland",
                geofabrik_path: "north-america/greenland",
            },
            Country {
                name: "California (US)",
                geofabrik_path: "north-america/us/california",
            },
            Country {
                name: "Texas (US)",
                geofabrik_path: "north-america/us/texas",
            },
            Country {
                name: "New York (US)",
                geofabrik_path: "north-america/us/new-york",
            },
            Country {
                name: "Florida (US)",
                geofabrik_path: "north-america/us/florida",
            },
            Country {
                name: "Washington (US)",
                geofabrik_path: "north-america/us/washington",
            },
        ],
    },
    Continent {
        name: "South America",
        countries: &[
            Country {
                name: "Brazil",
                geofabrik_path: "south-america/brazil",
            },
            Country {
                name: "Argentina",
                geofabrik_path: "south-america/argentina",
            },
            Country {
                name: "Chile",
                geofabrik_path: "south-america/chile",
            },
            Country {
                name: "Colombia",
                geofabrik_path: "south-america/colombia",
            },
            Country {
                name: "Peru",
                geofabrik_path: "south-america/peru",
            },
            Country {
                name: "Uruguay",
                geofabrik_path: "south-america/uruguay",
            },
            Country {
                name: "Venezuela",
                geofabrik_path: "south-america/venezuela",
            },
        ],
    },
    Continent {
        name: "Asia",
        countries: &[
            Country {
                name: "Japan",
                geofabrik_path: "asia/japan",
            },
            Country {
                name: "China",
                geofabrik_path: "asia/china",
            },
            Country {
                name: "India",
                geofabrik_path: "asia/india",
            },
            Country {
                name: "Singapore",
                geofabrik_path: "asia/singapore",
            },
            Country {
                name: "Thailand",
                geofabrik_path: "asia/thailand",
            },
            Country {
                name: "Vietnam",
                geofabrik_path: "asia/vietnam",
            },
            Country {
                name: "South Korea",
                geofabrik_path: "asia/south-korea",
            },
            Country {
                name: "Turkey",
                geofabrik_path: "asia/turkey",
            },
            Country {
                name: "Taiwan",
                geofabrik_path: "asia/taiwan",
            },
        ],
    },
    Continent {
        name: "Africa",
        countries: &[
            Country {
                name: "South Africa",
                geofabrik_path: "africa/south-africa",
            },
            Country {
                name: "Egypt",
                geofabrik_path: "africa/egypt",
            },
            Country {
                name: "Morocco",
                geofabrik_path: "africa/morocco",
            },
            Country {
                name: "Kenya",
                geofabrik_path: "africa/kenya",
            },
            Country {
                name: "Nigeria",
                geofabrik_path: "africa/nigeria",
            },
            Country {
                name: "Madagascar",
                geofabrik_path: "africa/madagascar",
            },
        ],
    },
    Continent {
        name: "Australia & Oceania",
        countries: &[
            Country {
                name: "Australia",
                geofabrik_path: "australia-oceania/australia",
            },
            Country {
                name: "New Zealand",
                geofabrik_path: "australia-oceania/new-zealand",
            },
            Country {
                name: "Fiji",
                geofabrik_path: "australia-oceania/fiji",
            },
        ],
    },
    Continent {
        name: "Central America",
        countries: &[
            Country {
                name: "Costa Rica",
                geofabrik_path: "central-america/costa-rica",
            },
            Country {
                name: "Panama",
                geofabrik_path: "central-america/panama",
            },
            Country {
                name: "Guatemala",
                geofabrik_path: "central-america/guatemala",
            },
            Country {
                name: "Belize",
                geofabrik_path: "central-america/belize",
            },
            Country {
                name: "Nicaragua",
                geofabrik_path: "central-america/nicaragua",
            },
        ],
    },
];
