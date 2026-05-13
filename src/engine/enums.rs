#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerColor {
    None = 0,
    Red = 1,
    Orange = 2,
    Yellow = 3,
    Blue = 4,
    Black = 5,
    Violet = 6,
    SkyBlue = 7,
    Green = 8,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerOrder {
    None = 0,
    Player1 = 1,
    Player2 = 2,
    Player3 = 3,
    Player4 = 4,
    Player5 = 5,
    Player6 = 6,
    Player7 = 7,
    Player8 = 8,
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Invalid = -1,
    Centre = 0,
    North = 1,
    NorthEast = 2,
    East = 3,
    SouthEast = 4,
    South = 5,
    SouthWest = 6,
    West = 7,
    NorthWest = 8,
    Base = 9,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UnitType(pub u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BuildingType(pub u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GoodsType(pub u16);

pub mod unit_type {
    pub const NULL: u16 = 0;
    pub const PEASANT: u16 = 1;
    pub const BURNING_MAN: u16 = 2;
    pub const WOODCUTTER: u16 = 3;
    pub const FLETCHER: u16 = 4;
    pub const TUNNELER: u16 = 5;
    pub const HUNTER: u16 = 6;
    pub const QUARRY_MASON: u16 = 7;
    pub const QUARRY_GRUNT: u16 = 8;
    pub const QUARRY_OX: u16 = 9;
    pub const PITCHMAN: u16 = 10;
    pub const FARMER_WHEAT: u16 = 11;
    pub const FARMER_HOPS: u16 = 12;
    pub const FARMER_APPLE: u16 = 13;
    pub const FARMER_CATTLE: u16 = 14;
    pub const MILLER: u16 = 15;
    pub const BAKER: u16 = 16;
    pub const BREWER: u16 = 17;
    pub const POLETURNER: u16 = 18;
    pub const BLACKSMITH: u16 = 19;
    pub const ARMOURER: u16 = 20;
    pub const TANNER: u16 = 21;
    pub const ARCHER: u16 = 22;
    pub const XBOWMAN: u16 = 23;
    pub const SPEARMAN: u16 = 24;
    pub const PIKEMAN: u16 = 25;
    pub const MACEMAN: u16 = 26;
    pub const SWORDSMAN: u16 = 27;
    pub const KNIGHT: u16 = 28;
    pub const LADDERMAN: u16 = 29;
    pub const ENGINEER: u16 = 30;
    pub const MINER1: u16 = 31;
    pub const MINER2: u16 = 32;
    pub const PRIEST: u16 = 33;
    pub const HEALER: u16 = 34;
    pub const DRUNKARD: u16 = 35;
    pub const INNKEEPER: u16 = 36;
    pub const MONK: u16 = 37;
    pub const ARCHER_DEBUG: u16 = 38;
    pub const CATAPULT: u16 = 39;
    pub const TREBUCHET: u16 = 40;
    pub const MANGONEL: u16 = 41;
    pub const TRADER: u16 = 42;
    pub const TRADER_HORSE: u16 = 43;
    pub const DEER: u16 = 44;
    pub const LION: u16 = 45;
    pub const RABBIT: u16 = 46;
    pub const CAMEL: u16 = 47;
    pub const CROW: u16 = 48;
    pub const SEAGULL: u16 = 49;
    pub const SIEGE_TENT: u16 = 50;
    pub const COW: u16 = 51;
    pub const DOG: u16 = 52;
    pub const FIREMAN: u16 = 53;
    pub const GHOST: u16 = 54;
    pub const LORD: u16 = 55;
    pub const LADY: u16 = 56;
    pub const JESTER: u16 = 57;
    pub const SIEGE_TOWER: u16 = 58;
    pub const BATTERING_RAM: u16 = 59;
    pub const PORTABLE_SHIELD: u16 = 60;
    pub const BALLISTA: u16 = 61;
    pub const CHICKEN: u16 = 62;
    pub const MOTHER: u16 = 63;
    pub const CHILD: u16 = 64;
    pub const JUGGLER: u16 = 65;
    pub const FIREEATER: u16 = 66;
    pub const WAR_DOG: u16 = 67;
    pub const BURNING_ANIMAL_BIG: u16 = 68;
    pub const BURNING_ANIMAL_SMALL: u16 = 69;
    pub const ARAB_BOW: u16 = 70;
    pub const ARAB_SLAVE: u16 = 71;
    pub const ARAB_SLINGER: u16 = 72;
    pub const ARAB_ASSASIN: u16 = 73;
    pub const ARAB_HORSEMAN: u16 = 74;
    pub const ARAB_SWORDSMAN: u16 = 75;
    pub const ARAB_GRENADIER: u16 = 76;
    pub const ARAB_BALLISTA: u16 = 77;
    pub const BEDOUIN_CAMEL_LANCER: u16 = 78;
    pub const BEDOUIN_HEALER: u16 = 79;
    pub const BEDOUIN_EUNUCH: u16 = 80;
    pub const BEDOUIN_AMBUSHER: u16 = 81;
    pub const BEDOUIN_SKIRMISHER: u16 = 82;
    pub const BEDOUIN_HEAVY_CAMEL: u16 = 83;
    pub const BEDOUIN_SAPPER: u16 = 84;
    pub const BEDOUIN_DEMOLISHER: u16 = 85;
    pub const GOAT: u16 = 86;
    pub const HYENA: u16 = 87;
    pub const CROCODILE: u16 = 88;
    pub const NUM_TYPES: u16 = 89;
}

pub mod building_type {
    pub const NULL: u16 = 0;
    pub const HOVEL: u16 = 1;
    pub const OUTPOST_BEDOUIN: u16 = 2;
    pub const WOODCUTTERS_HUT: u16 = 3;
    pub const OXEN_BASE: u16 = 4;
    pub const IRON_MINE: u16 = 5;
    pub const PITCH_DIGGER: u16 = 6;
    pub const HUNTERS_HUT: u16 = 7;
    pub const BARRACKS_WOOD: u16 = 8;
    pub const BARRACKS_STONE: u16 = 9;
    pub const GOODS_YARD: u16 = 10;
    pub const ARMOURY: u16 = 11;
    pub const FLETCHERS_WORKSHOP: u16 = 12;
    pub const BLACKSMITHS_WORKSHOP: u16 = 13;
    pub const POLETURNERS_WORKSHOP: u16 = 14;
    pub const ARMOURERS_WORKSHOP: u16 = 15;
    pub const TANNERS_WORKSHOP: u16 = 16;
    pub const BAKERS_WORKSHOP: u16 = 17;
    pub const BREWERS_WORKSHOP: u16 = 18;
    pub const GRANARY: u16 = 19;
    pub const QUARRY: u16 = 20;
    pub const QUARRYPILE: u16 = 21;
    pub const INN: u16 = 22;
    pub const HEALER: u16 = 23;
    pub const ENGINEERS_GUILD: u16 = 24;
    pub const TUNNELLERS_GUILD: u16 = 25;
    pub const TRADEPOST: u16 = 26;
    pub const WELL: u16 = 27;
    pub const OIL_SMELTER: u16 = 28;
    pub const SIEGE_TENT: u16 = 29;
    pub const WHEATFARM: u16 = 30;
    pub const HOPSFARM: u16 = 31;
    pub const APPLEFARM: u16 = 32;
    pub const CATTLEFARM: u16 = 33;
    pub const MILL: u16 = 34;
    pub const STABLES: u16 = 35;
    pub const CHURCH1: u16 = 36;
    pub const CHURCH2: u16 = 37;
    pub const CHURCH3: u16 = 38;
    pub const RUINS: u16 = 39;
    pub const KEEP_ONE: u16 = 40;
    pub const KEEP_TWO: u16 = 41;
    pub const KEEP_THREE: u16 = 42;
    pub const KEEP_FOUR: u16 = 43;
    pub const KEEP_FIVE: u16 = 44;
    pub const GATE_MAIN: u16 = 45;
    pub const GATE_INNER: u16 = 46;
    pub const GATE_WOOD: u16 = 47;
    pub const GATE_POSTERN: u16 = 48;
    pub const DRAWBRIDGE: u16 = 49;
    pub const TUNNEL_ENTERANCE: u16 = 50;
    pub const PARADEGROUND_OIL: u16 = 51;
    pub const SIGNPOST: u16 = 52;
    pub const PARADEGROUND_ENG: u16 = 53;
    pub const SIEGE_TENT_ARAB_BALLISTA: u16 = 54;
    pub const CAMPGROUND: u16 = 55;
    pub const PARADEGROUND_MISS: u16 = 56;
    pub const PARADEGROUND_LGT: u16 = 57;
    pub const PARADEGROUND_HVY: u16 = 58;
    pub const PARADEGROUND_TUN: u16 = 59;
    pub const GATEHOUSE: u16 = 60;
    pub const TOWER: u16 = 61;
    pub const GALLOWS: u16 = 62;
    pub const STOCKS: u16 = 63;
    pub const WITCH_HOIST: u16 = 64;
    pub const MAYPOLE: u16 = 65;
    pub const GARDEN: u16 = 66;
    pub const KILLING_PIT: u16 = 67;
    pub const PITCH_DITCH: u16 = 68;
    pub const SIEGE_TOWER: u16 = 69;
    pub const WATERPOT: u16 = 70;
    pub const KEEPDOOR_LEFT: u16 = 71;
    pub const KEEPDOOR_RIGHT: u16 = 72;
    pub const KEEPDOOR: u16 = 73;
    pub const TOWER1: u16 = 74;
    pub const TOWER2: u16 = 75;
    pub const TOWER3: u16 = 76;
    pub const TOWER4: u16 = 77;
    pub const TOWER5: u16 = 78;
    pub const TOWER5_DESTROYED: u16 = 79;
    pub const SIEGE_TENT_CATAPULT: u16 = 80;
    pub const SIEGE_TENT_TREBUCHET: u16 = 81;
    pub const SIEGE_TENT_SIEGE_TOWER: u16 = 82;
    pub const SIEGE_TENT_BATTERING_RAM: u16 = 83;
    pub const SIEGE_TENT_PORTABLE_SHIELD: u16 = 84;
    pub const TUNNEL_CONSTRUCTION: u16 = 85;
    pub const TOWER1_DESTROYED: u16 = 86;
    pub const TOWER2_DESTROYED: u16 = 87;
    pub const TOWER3_DESTROYED: u16 = 88;
    pub const TOWER4_DESTROYED: u16 = 89;
    pub const WAS_WALL: u16 = 90;
    pub const CESS_PIT: u16 = 91;
    pub const BURNING_STAKE: u16 = 92;
    pub const GIBBET: u16 = 93;
    pub const DUNGEON: u16 = 94;
    pub const RACK_STRETCHING: u16 = 95;
    pub const RACK_FLOGGING: u16 = 96;
    pub const CHOPPING_BLOCK: u16 = 97;
    pub const DUNKING_STOOL: u16 = 98;
    pub const DOG_CAGE: u16 = 99;
    pub const STATUE: u16 = 100;
    pub const SHRINE: u16 = 101;
    pub const BEE_HIVE: u16 = 102;
    pub const DANCING_BEAR: u16 = 103;
    pub const POND: u16 = 104;
    pub const BEAR_CAVE: u16 = 105;
    pub const OUTPOST: u16 = 106;
    pub const OUTPOST_ARAB: u16 = 107;
    pub const BEDOUIN_STOCKADE: u16 = 108;
    pub const DOCK: u16 = 109;
    pub const MAX: u16 = 110;
    pub const WOOD_WALL: u16 = 110;
    pub const STONE_WALL: u16 = 111;
    pub const CRENAL_WALL: u16 = 112;
    pub const STAIRS: u16 = 113;
    pub const BRAZIER: u16 = 114;
    pub const MANGONEL: u16 = 115;
    pub const BALLISTA: u16 = 116;
    pub const HEAD_ON_SPIKE: u16 = 117;
    pub const GARDEN_SMALL: u16 = 118;
    pub const GARDEN_MED: u16 = 119;
    pub const GARDEN_LARGE: u16 = 120;
    pub const POND_SMALL: u16 = 121;
    pub const POND_LARGE: u16 = 122;
    pub const FLAG1: u16 = 123;
    pub const FLAG2: u16 = 124;
    pub const FLAG3: u16 = 125;
    pub const FLAG4: u16 = 126;
    pub const GATE_WOOD1A: u16 = 127;
    pub const GATE_WOOD1B: u16 = 128;
    pub const GATE_WOOD1C: u16 = 129;
    pub const GATE_WOOD1D: u16 = 130;
    pub const GATE_STONE1A: u16 = 131;
    pub const GATE_STONE1B: u16 = 132;
    pub const GATE_STONE2A: u16 = 133;
    pub const GATE_STONE2B: u16 = 134;
    pub const RUINS01: u16 = 135;
    pub const RUINS02: u16 = 136;
    pub const RUINS03: u16 = 137;
    pub const RUINS04: u16 = 138;
    pub const RUINS05: u16 = 139;
    pub const RUINS06: u16 = 140;
    pub const RUINS07: u16 = 141;
    pub const RUINS08: u16 = 142;
    pub const RUINS09: u16 = 143;
    pub const RUINS10: u16 = 144;
    pub const RUINS11: u16 = 145;
    pub const RUINS12: u16 = 146;
    pub const RUINS13: u16 = 147;
    pub const PEOPLE_ARCHERS: u16 = 148;
    pub const PEOPLE_SPEARMEN: u16 = 149;
    pub const PEOPLE_PIKEMEN: u16 = 150;
    pub const PEOPLE_MACEMEN: u16 = 151;
    pub const PEOPLE_XBOWMEN: u16 = 152;
    pub const PEOPLE_SWORDSMEN: u16 = 153;
    pub const PEOPLE_KNIGHTS: u16 = 154;
    pub const PEOPLE_LADDERMEN: u16 = 155;
    pub const PEOPLE_ENGINEERS: u16 = 156;
    pub const PEOPLE_ENGINEERS_POTS: u16 = 157;
    pub const PEOPLE_MONKS: u16 = 158;
    pub const PEOPLE_CATAPULTS: u16 = 159;
    pub const PEOPLE_TREBUCHETS: u16 = 160;
    pub const PEOPLE_BATTERING_RAMS: u16 = 161;
    pub const PEOPLE_SIEGE_TOWERS: u16 = 162;
    pub const PEOPLE_PORTABLE_SHIELDS: u16 = 163;
    pub const PEOPLE_TUNNELERS: u16 = 164;
    pub const NEW_DIG_MOAT: u16 = 168;
    pub const NEW_FILL_MOAT: u16 = 169;
    pub const MARKER_POINT1: u16 = 170;
    pub const MARKER_POINT2: u16 = 171;
    pub const MARKER_POINT3: u16 = 172;
    pub const MARKER_POINT4: u16 = 173;
    pub const MARKER_POINT5: u16 = 174;
    pub const MARKER_POINT6: u16 = 175;
    pub const MARKER_POINT7: u16 = 176;
    pub const MARKER_POINT8: u16 = 177;
    pub const MARKER_POINT9: u16 = 178;
    pub const MARKER_POINT10: u16 = 179;
    pub const RUINS14: u16 = 180;
    pub const RUINS15: u16 = 181;
    pub const RUINS16: u16 = 182;
    pub const RUINS17: u16 = 183;
    pub const POND5: u16 = 184;
    pub const POND6: u16 = 185;
    pub const POND7: u16 = 186;
    pub const POND8: u16 = 187;
    pub const IN_REPORTS: u16 = 190;
    pub const SUB_MENU_TOWERS: u16 = 200;
    pub const SUB_MENU_MILITARY: u16 = 201;
    pub const SUB_MENU_GATEHOUSES: u16 = 202;
    pub const SUB_MENU_KEEPS: u16 = 203;
    pub const SUB_MENU_GATEHOUSES_WOOD: u16 = 204;
    pub const SUB_MENU_GATEHOUSES_STONESMALL: u16 = 205;
    pub const SUB_MENU_GATEHOUSES_STONELARGE: u16 = 206;
    pub const SUB_MENU_GOOD: u16 = 207;
    pub const SUB_MENU_BAD: u16 = 208;
    pub const NEW_EDITOR_DELETE: u16 = 209;
    pub const MENU_RETURN_TOWERS: u16 = 210;
    pub const MENU_RETURN_GATEHOUSES: u16 = 211;
    pub const MENU_RETURN_MILITARY: u16 = 212;
    pub const MENU_RETURN_KEEPS: u16 = 213;
    pub const MENU_RETURN_GOOD: u16 = 214;
    pub const MENU_RETURN_BAD: u16 = 215;
    pub const NEW_DELETE: u16 = 216;
    pub const PEOPLE_ARAB_BOW: u16 = 220;
    pub const PEOPLE_ARAB_SLAVE: u16 = 221;
    pub const PEOPLE_ARAB_SLINGER: u16 = 222;
    pub const PEOPLE_ARAB_ASSASIN: u16 = 223;
    pub const PEOPLE_ARAB_HORSEMAN: u16 = 224;
    pub const PEOPLE_ARAB_SWORDSMAN: u16 = 225;
    pub const PEOPLE_ARAB_GRENADIER: u16 = 226;
    pub const PEOPLE_ARAB_BALLISTA: u16 = 227;
    pub const RUINS18: u16 = 230;
    pub const RUINS19: u16 = 231;
    pub const RUINS20: u16 = 232;
    pub const RUINS21: u16 = 233;
    pub const RUINS22: u16 = 234;
    pub const RUINS23: u16 = 235;
    pub const RUINS24: u16 = 236;
    pub const RUINS25: u16 = 237;
    pub const RUINS26: u16 = 238;
    pub const RUINS27: u16 = 239;
    pub const RUINS28: u16 = 240;
    pub const RUINS29: u16 = 241;
    pub const RUINS30: u16 = 242;
    pub const RUINS31: u16 = 243;
    pub const RUINS32: u16 = 244;
    pub const RUINS33: u16 = 245;
    pub const RUINS34: u16 = 246;
    pub const PEOPLE_BEDOUIN_CAMEL_LANCER: u16 = 247;
    pub const PEOPLE_BEDOUIN_HEALER: u16 = 248;
    pub const PEOPLE_BEDOUIN_EUNUCH: u16 = 249;
    pub const PEOPLE_BEDOUIN_AMBUSHER: u16 = 250;
    pub const PEOPLE_BEDOUIN_SKIRMISHER: u16 = 251;
    pub const PEOPLE_BEDOUIN_HEAVY_CAMEL: u16 = 252;
    pub const PEOPLE_BEDOUIN_SAPPER: u16 = 253;
    pub const PEOPLE_BEDOUIN_DEMOLISHER: u16 = 254;
}

pub mod veg_type {
    pub const NONE: i16 = 0x0;
    pub const OLIVE_TREE: i16 = 0x1;
    pub const DATE_PALM: i16 = 0x2;
    pub const COCO_PALM: i16 = 0x3;
    pub const CHERRY_TREE: i16 = 0x4;
    pub const DESERT_SHRUB1: i16 = 0x5;
    pub const DESERT_SHRUB1_VAR2: i16 = 0x6;
    pub const DESERT_SHRUB1_VAR3: i16 = 0x7;
    pub const DESERT_SHRUB1_VAR4: i16 = 0x8;
    pub const DESERT_SHRUB1_VAR5: i16 = 0x9;
    pub const CACTUS1: i16 = 0xA;
    pub const UNUSED1: i16 = 0xB;
    pub const UNUSED2: i16 = 0xC;
    pub const UNUSED3: i16 = 0xD;
    pub const UNUSED4: i16 = 0xE;
    pub const APPLE_TREE: i16 = 0xF;
    pub const CACTUS2: i16 = 0x10;
    pub const CACTUS3: i16 = 0x11;
    pub const DESERT_SHRUB2: i16 = 0x12;
    pub const CACTUS4: i16 = 0x13;
}

pub mod goods {
    pub const NULL: u16 = 0;
    pub const WOOD_LOGS: u16 = 1;
    pub const WOOD_PLANKS: u16 = 2;
    pub const RAW_HOPS: u16 = 3;
    pub const STONE_BLOCKS: u16 = 4;
    pub const COW_HIDES: u16 = 5;
    pub const IRON_INGOTS: u16 = 6;
    pub const PITCH_RAW: u16 = 7;
    pub const PITCH_REFINED: u16 = 8;
    pub const RAW_WHEAT: u16 = 9;
    pub const FOOD_BREAD: u16 = 10;
    pub const FOOD_CHEESE: u16 = 11;
    pub const FOOD_MEAT: u16 = 12;
    pub const FOOD_FRUIT: u16 = 13;
    pub const FOOD_ALE: u16 = 14;
    pub const GOLD: u16 = 15;
    pub const FLOUR: u16 = 16;
    pub const BOWS: u16 = 17;
    pub const CROSSBOWS: u16 = 18;
    pub const SPEARS: u16 = 19;
    pub const PIKES: u16 = 20;
    pub const MACES: u16 = 21;
    pub const SWORDS: u16 = 22;
    pub const LEATHER_ARMOUR: u16 = 23;
    pub const METAL_ARMOUR: u16 = 24;
    pub const COUNT: u16 = 25;
}

pub fn is_population_unit(type_id: u16) -> bool {
    matches!(
        type_id,
        unit_type::PEASANT
            | unit_type::WOODCUTTER
            | unit_type::FLETCHER
            | unit_type::HUNTER
            | unit_type::QUARRY_MASON
            | unit_type::QUARRY_GRUNT
            | unit_type::PITCHMAN
            | unit_type::FARMER_WHEAT
            | unit_type::FARMER_HOPS
            | unit_type::FARMER_APPLE
            | unit_type::FARMER_CATTLE
            | unit_type::MILLER
            | unit_type::BAKER
            | unit_type::BREWER
            | unit_type::POLETURNER
            | unit_type::BLACKSMITH
            | unit_type::ARMOURER
            | unit_type::TANNER
            | unit_type::INNKEEPER
            | unit_type::FIREMAN
            | unit_type::HEALER
            | unit_type::PRIEST
    )
}
