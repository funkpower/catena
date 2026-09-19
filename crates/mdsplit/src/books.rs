//! The canonical books of scripture, in order.
//!
//! General reference data rather than anything local: the order is the
//! canonical one, so a section index taken from it sorts the way a
//! reader expects. Primaries are the Septuagint names, with the English
//! and Protestant spellings as variants, because an edition uses one or
//! the other throughout and both have to resolve to the same book.

/// Each book, with every spelling that means it.
pub static BOOKS: &[(&str, &[&str])] = &[
    ("Genesis", &["Genesis", "Gen"]),
    ("Exodus", &["Exodus", "Exod", "Exo"]),
    ("Leviticus", &["Leviticus", "Lev"]),
    ("Numbers", &["Numbers", "Num"]),
    ("Deuteronomy", &["Deuteronomy", "Deut", "Dt"]),
    ("Joshua", &["Joshua", "Josh"]),
    ("Judges", &["Judges", "Judg"]),
    ("Ruth", &["Ruth"]),
    (
        "1 Kingdoms",
        &[
            "1 Kingdoms",
            "1 Samuel",
            "First Samuel",
            "I Samuel",
            "1 Sam",
        ],
    ),
    (
        "2 Kingdoms",
        &[
            "2 Kingdoms",
            "2 Samuel",
            "Second Samuel",
            "II Samuel",
            "2 Sam",
        ],
    ),
    (
        "3 Kingdoms",
        &["3 Kingdoms", "1 Kings", "First Kings", "I Kings", "1 Kgs"],
    ),
    (
        "4 Kingdoms",
        &["4 Kingdoms", "2 Kings", "Second Kings", "II Kings", "2 Kgs"],
    ),
    (
        "1 Chronicles",
        &[
            "1 Chronicles",
            "First Chronicles",
            "I Chronicles",
            "1 Chron",
            "1 Chr",
        ],
    ),
    (
        "2 Chronicles",
        &[
            "2 Chronicles",
            "Second Chronicles",
            "II Chronicles",
            "2 Chron",
            "2 Chr",
        ],
    ),
    ("1 Ezra", &["1 Ezra", "1 Esdras", "First Esdras"]),
    ("2 Ezra", &["2 Ezra", "2 Esdras", "Second Esdras", "Ezra"]),
    ("Nehemiah", &["Nehemiah", "Neh"]),
    ("Tobit", &["Tobit", "Tob"]),
    ("Judith", &["Judith", "Jdt"]),
    ("Esther", &["Esther", "Est"]),
    (
        "1 Maccabees",
        &["1 Maccabees", "First Maccabees", "I Maccabees", "1 Macc"],
    ),
    (
        "2 Maccabees",
        &["2 Maccabees", "Second Maccabees", "II Maccabees", "2 Macc"],
    ),
    (
        "3 Maccabees",
        &["3 Maccabees", "Third Maccabees", "III Maccabees", "3 Macc"],
    ),
    (
        "4 Maccabees",
        &["4 Maccabees", "Fourth Maccabees", "IV Maccabees", "4 Macc"],
    ),
    ("Psalm", &["Psalm", "Psalms", "Ps"]),
    ("Job", &["Job"]),
    ("Proverbs", &["Proverbs", "Prov", "Prv"]),
    ("Ecclesiastes", &["Ecclesiastes", "Eccl", "Eccles"]),
    (
        "Song of Songs",
        &[
            "Song of Songs",
            "Song of Solomon",
            "Canticles",
            "Canticle of Canticles",
            "Cant",
        ],
    ),
    ("Wisdom of Solomon", &["Wisdom of Solomon", "Wisdom", "Wis"]),
    (
        "Wisdom of Sirach",
        &["Wisdom of Sirach", "Sirach", "Ecclesiasticus", "Sir"],
    ),
    ("Hosea", &["Hosea", "Hos"]),
    ("Amos", &["Amos"]),
    ("Micah", &["Micah", "Mic"]),
    ("Joel", &["Joel"]),
    ("Obadiah", &["Obadiah", "Obad", "Ob"]),
    ("Jonah", &["Jonah", "Jon"]),
    ("Nahum", &["Nahum", "Nah"]),
    ("Habakkuk", &["Habakkuk", "Hab"]),
    ("Zephaniah", &["Zephaniah", "Zeph", "Zep"]),
    ("Haggai", &["Haggai", "Hag"]),
    ("Zechariah", &["Zechariah", "Zech", "Zec"]),
    ("Malachi", &["Malachi", "Mal"]),
    ("Isaiah", &["Isaiah", "Isa"]),
    ("Jeremiah", &["Jeremiah", "Jer"]),
    ("Baruch", &["Baruch", "Bar"]),
    (
        "Lamentations",
        &["Lamentations of Jeremiah", "Lamentations", "Lam"],
    ),
    (
        "Epistle of Jeremiah",
        &["Epistle of Jeremiah", "Letter of Jeremiah"],
    ),
    ("Ezekiel", &["Ezekiel", "Ezek", "Ezk"]),
    ("Daniel", &["Daniel", "Dan"]),
    ("Prayer of Manasseh", &["Prayer of Manasseh", "Pr Man"]),
    ("Matthew", &["Matthew", "Matt", "Mt"]),
    ("Mark", &["Mark", "Mk"]),
    ("Luke", &["Luke", "Lk"]),
    ("John", &["John", "Jn"]),
    ("Acts", &["Acts"]),
    ("Romans", &["Romans", "Rom"]),
    (
        "1 Corinthians",
        &[
            "1 Corinthians",
            "First Corinthians",
            "I Corinthians",
            "1 Cor",
        ],
    ),
    (
        "2 Corinthians",
        &[
            "2 Corinthians",
            "Second Corinthians",
            "II Corinthians",
            "2 Cor",
        ],
    ),
    ("Galatians", &["Galatians", "Gal"]),
    ("Ephesians", &["Ephesians", "Eph"]),
    ("Philippians", &["Philippians", "Phil"]),
    ("Colossians", &["Colossians", "Col"]),
    (
        "1 Thessalonians",
        &[
            "1 Thessalonians",
            "First Thessalonians",
            "I Thessalonians",
            "1 Thess",
        ],
    ),
    (
        "2 Thessalonians",
        &[
            "2 Thessalonians",
            "Second Thessalonians",
            "II Thessalonians",
            "2 Thess",
        ],
    ),
    (
        "1 Timothy",
        &["1 Timothy", "First Timothy", "I Timothy", "1 Tim"],
    ),
    (
        "2 Timothy",
        &["2 Timothy", "Second Timothy", "II Timothy", "2 Tim"],
    ),
    ("Titus", &["Titus"]),
    ("Philemon", &["Philemon", "Phlm"]),
    ("Hebrews", &["Hebrews", "Heb"]),
    ("James", &["James", "Jas"]),
    ("1 Peter", &["1 Peter", "First Peter", "I Peter", "1 Pet"]),
    ("2 Peter", &["2 Peter", "Second Peter", "II Peter", "2 Pet"]),
    ("1 John", &["1 John", "First John", "I John", "1 Jn"]),
    ("2 John", &["2 John", "Second John", "II John", "2 Jn"]),
    ("3 John", &["3 John", "Third John", "III John", "3 Jn"]),
    ("Jude", &["Jude"]),
    ("Revelation", &["Revelation", "Apocalypse", "Rev", "Apoc"]),
];
