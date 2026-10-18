//! Read-only multiplayer appearance import using the game's existing managed member list.
use super::addresses::mono_offsets as layout;
use anyhow::{bail, ensure, Context, Result};
use std::collections::BTreeMap;

pub trait Memory {
    fn read(&self, address: usize, length: usize) -> Result<Vec<u8>>;
}

#[derive(Clone, Debug, Default)]
pub struct PlayerIdentity {
    pub name: Option<String>,
    pub color: Option<i32>,
}

#[derive(Clone, Debug, Default)]
pub struct MultiplayerPlayers {
    pub players: BTreeMap<usize, PlayerIdentity>,
}

impl MultiplayerPlayers {
    /// Imported names fall back to numbered slots; unknown colors retain the manual choice.
    pub fn appearance(&self, current_colors: [i32; 8]) -> [(String, i32); 8] {
        std::array::from_fn(|index| {
            let player = self.players.get(&(index + 1));
            let name = player
                .and_then(|p| p.name.as_deref())
                .filter(|name| !name.trim().is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| format!("Player {}", index + 1));
            let color = player
                .and_then(|p| p.color)
                .filter(|c| (1..=8).contains(c))
                .unwrap_or(current_colors[index]);
            (name, color)
        })
    }
    pub fn name_count(&self) -> usize {
        self.players.values().filter(|p| p.name.is_some()).count()
    }
    pub fn color_count(&self) -> usize {
        self.players.values().filter(|p| p.color.is_some()).count()
    }
}

struct Reader<'a, M>(&'a M);
impl<M: Memory> Reader<'_, M> {
    fn bytes(&self, address: usize, length: usize) -> Result<Vec<u8>> {
        ensure!(
            (0x10000..0x8000_0000_0000).contains(&address),
            "Invalid Mono pointer"
        );
        ensure!(
            length <= 1024 * 1024
                && address
                    .checked_add(length)
                    .is_some_and(|end| end < 0x8000_0000_0000),
            "Invalid Mono read size"
        );
        let bytes = self.0.read(address, length)?;
        ensure!(bytes.len() == length, "Incomplete Mono read");
        Ok(bytes)
    }
    fn ptr(&self, address: usize) -> Result<usize> {
        Ok(u64::from_le_bytes(self.bytes(address, 8)?.as_slice().try_into()?) as usize)
    }
    fn i32(&self, address: usize) -> Result<i32> {
        Ok(i32::from_le_bytes(
            self.bytes(address, 4)?.as_slice().try_into()?,
        ))
    }
    fn byte(&self, address: usize) -> Result<u8> {
        Ok(self.bytes(address, 1)?[0])
    }
    fn cstring(&self, address: usize) -> Result<String> {
        let mut output = Vec::new();
        while output.len() < 256 {
            let position = address
                .checked_add(output.len())
                .context("Mono string overflow")?;
            let count = 64.min(0x1000 - (position & 0xfff)).min(256 - output.len());
            let bytes = self.bytes(position, count)?;
            if let Some(end) = bytes.iter().position(|b| *b == 0) {
                output.extend_from_slice(&bytes[..end]);
                return String::from_utf8(output).context("Invalid Mono metadata name");
            }
            output.extend(bytes);
        }
        bail!("Unterminated Mono metadata name")
    }
    fn class(&self, object: usize) -> Result<usize> {
        self.ptr(self.ptr(object)?)
    }
    fn class_name(&self, class: usize) -> Result<(String, String)> {
        Ok((
            self.cstring(self.ptr(class + layout::CLASS_NAMESPACE)?)?,
            self.cstring(self.ptr(class + layout::CLASS_NAME)?)?,
        ))
    }
    fn fields(&self, class: usize) -> Result<BTreeMap<String, usize>> {
        let mut definition = class;
        if self.byte(class + layout::CLASS_KIND)? == 3 {
            definition = self.ptr(self.ptr(class + layout::CLASS_GENERIC)?)?;
        }
        ensure!(
            [1, 2].contains(&self.byte(definition + layout::CLASS_KIND)?),
            "Unsupported Mono class kind"
        );
        let count = self.i32(definition + layout::CLASS_FIELD_COUNT)?;
        ensure!((0..=512).contains(&count), "Invalid Mono field count");
        let start = self.ptr(class + layout::CLASS_FIELDS)?;
        let bytes = self.bytes(start, count as usize * layout::FIELD_SIZE)?;
        let mut fields = BTreeMap::new();
        for field in bytes.chunks_exact(layout::FIELD_SIZE) {
            let name = self.cstring(u64::from_le_bytes(
                field[layout::FIELD_NAME..layout::FIELD_NAME + 8].try_into()?,
            ) as usize)?;
            let offset = i32::from_le_bytes(
                field[layout::FIELD_OFFSET..layout::FIELD_OFFSET + 4].try_into()?,
            );
            // Negative offsets are thread-static/special fields, never ordinary object data.
            if (0..=1024 * 1024).contains(&offset) {
                fields.insert(name, offset as usize);
            }
        }
        Ok(fields)
    }
    fn string(&self, object: usize) -> Result<String> {
        if object == 0 {
            return Ok(String::new());
        }
        let (namespace, name) = self.class_name(self.class(object)?)?;
        ensure!(
            namespace == "System" && name == "String",
            "Player name is not a Mono string"
        );
        let count = self.i32(object + layout::STRING_LENGTH)?;
        ensure!((0..=256).contains(&count), "Invalid player name length");
        let bytes = self.bytes(object + layout::STRING_DATA, count as usize * 2)?;
        ensure!(
            self.i32(object + layout::STRING_LENGTH)? == count,
            "Player name changed during read"
        );
        let utf16: Vec<_> = bytes
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .collect();
        let text = String::from_utf16(&utf16).context("Invalid player-name UTF-16")?;
        Ok(text
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect::<String>()
            .trim()
            .to_owned())
    }
}

fn required(fields: &BTreeMap<String, usize>, name: &str) -> Result<usize> {
    fields
        .get(name)
        .copied()
        .with_context(|| format!("Mono field {name} is not initialized"))
}

pub struct Runtime {
    root_global: usize,
    root: usize,
    platform: usize,
    member: usize,
    instance_offset: usize,
    members_offset: usize,
    player_id_offset: usize,
    player_name_offset: usize,
    colour_offset: usize,
    ai_offset: usize,
}

impl Runtime {
    pub fn locate(memory: &impl Memory, base: usize, file: &[u8]) -> Result<Self> {
        let reader = Reader(memory);
        // This compatibility profile is verified against the shipped x64 Unity Mono.
        // Require all metadata-layout accessors to match before using internal offsets.
        let anchors: &[(usize, &[u8])] = &[
            (
                layout::ACCESSOR_0,
                &[0x48, 0x8b, 0x05, 0x69, 0xae, 0x6a, 0x00, 0xc3],
            ),
            (
                layout::ACCESSOR_1,
                &[0x48, 0x8b, 0x1d, 0x54, 0xad, 0x69, 0x00],
            ),
            (layout::ACCESSOR_2, &[0x48, 0x8b, 0x41, 0x30, 0xc3]),
            (layout::ACCESSOR_3, &[0x48, 0x8b, 0x7f, 0x48]),
            (layout::ACCESSOR_4, &[0x48, 0x8b, 0x41, 0x08, 0xc3]),
            (layout::ACCESSOR_5, &[0x8b, 0x43, 0x18]),
            (
                layout::ACCESSOR_6,
                &[0x48, 0x8b, 0xb7, 0xd0, 0x00, 0x00, 0x00],
            ),
            (
                layout::ACCESSOR_7,
                &[
                    0xf6, 0x41, 0x30, 0x04, 0x75, 0x03, 0x33, 0xc0, 0xc3, 0x48, 0x8b, 0x01, 0x48,
                    0x63, 0x40, 0x5c, 0x48, 0x8b, 0x44, 0xc1, 0x48, 0xc3,
                ],
            ),
            (layout::ACCESSOR_8, &[0x8b, 0x41, 0x10, 0xc3]),
            (layout::ACCESSOR_9, &[0x8b, 0x41, 0x18, 0xc3]),
        ];
        for &(rva, expected) in anchors {
            ensure!(
                file_slice(file, rva, expected.len())? == expected
                    && reader.bytes(base + rva, expected.len())? == expected,
                "Unsupported Mono runtime layout"
            );
        }
        Self::discover(
            memory,
            base.checked_add(layout::ROOT_GLOBAL)
                .context("Mono module overflow")?,
            base.checked_add(layout::ASSEMBLIES_GLOBAL)
                .context("Mono module overflow")?,
        )
    }

    fn discover(
        memory: &impl Memory,
        root_global: usize,
        assemblies_global: usize,
    ) -> Result<Self> {
        let reader = Reader(memory);
        let root = reader.ptr(root_global)?;
        let mut node = reader.ptr(assemblies_global)?;
        let mut image = None;
        let mut visited = std::collections::BTreeSet::new();
        while node != 0 {
            ensure!(
                visited.len() < 512 && visited.insert(node),
                "Invalid Mono assembly chain"
            );
            let assembly = reader.ptr(node)?;
            let candidate = reader.ptr(assembly + layout::ASSEMBLY_IMAGE)?;
            if matches!(
                reader
                    .cstring(reader.ptr(candidate + layout::IMAGE_NAME)?)?
                    .as_str(),
                "Assembly-CSharp" | "Assembly-CSharp.dll"
            ) {
                image = Some(candidate);
                break;
            }
            node = reader.ptr(node + 8)?;
        }
        let image = image.context("Game managed assembly is not loaded")?;
        let cache = image + layout::IMAGE_CACHE;
        let bucket_count = reader.i32(cache + layout::CACHE_COUNT)?;
        ensure!(
            (1..=65536).contains(&bucket_count),
            "Invalid Mono class cache size"
        );
        let buckets = reader.ptr(cache + layout::CACHE_BUCKETS)?;
        // The cache provides its own next-node accessor. Read its instructions; never call it.
        let next_accessor = reader.ptr(cache + layout::CACHE_NEXT_ACCESSOR)?;
        let next_code = reader.bytes(next_accessor, 8)?;
        ensure!(
            next_code == [0x48, 0x8d, 0x81, 0x08, 0x01, 0x00, 0x00, 0xc3],
            "Unsupported Mono class-cache links"
        );
        let mut platform = None;
        let mut member = None;
        let heads = reader.bytes(buckets, bucket_count as usize * 8)?;
        let mut visited = std::collections::BTreeSet::new();
        for bucket in heads.chunks_exact(8) {
            let mut class = u64::from_le_bytes(bucket.try_into()?) as usize;
            while class != 0 {
                ensure!(
                    visited.len() < 20000 && visited.insert(class),
                    "Invalid Mono class-cache chain"
                );
                let (namespace, name) = reader.class_name(class)?;
                if namespace.is_empty() && name == "Platform_Multiplayer" {
                    platform = Some(class);
                }
                if namespace.is_empty() && name == "MPGameMember" {
                    member = Some(class);
                }
                if platform.is_some() && member.is_some() {
                    break;
                }
                class = reader.ptr(class + layout::CLASS_NEXT)?;
            }
            if platform.is_some() && member.is_some() {
                break;
            }
        }
        let platform = platform.context("Multiplayer class is not initialized yet")?;
        let member = member.context("Multiplayer member class is not initialized yet")?;
        ensure!(
            reader.ptr(platform + layout::CLASS_IMAGE)? == image
                && reader.ptr(member + layout::CLASS_IMAGE)? == image
                && reader.ptr(member + layout::CLASS_NESTED_PARENT)? == platform,
            "Unexpected multiplayer class identity"
        );
        let platform_fields = reader.fields(platform)?;
        let member_fields = reader.fields(member)?;
        Ok(Self {
            root_global,
            root,
            platform,
            member,
            instance_offset: required(&platform_fields, "instance")?,
            members_offset: required(&platform_fields, "gameMembers")?,
            player_id_offset: required(&member_fields, "playerID")?,
            player_name_offset: required(&member_fields, "playerName")?,
            colour_offset: required(&member_fields, "colourID")?,
            ai_offset: required(&member_fields, "skirmishAI")?,
        })
    }

    pub fn players(&self, memory: &impl Memory) -> Result<MultiplayerPlayers> {
        let reader = Reader(memory);
        ensure!(
            reader.ptr(self.root_global)? == self.root,
            "Mono domain changed"
        );
        let domain_id = reader.i32(self.root + layout::DOMAIN_ID)?;
        ensure!((0..=64).contains(&domain_id), "Invalid Mono domain ID");
        let info = reader.ptr(self.platform + layout::CLASS_RUNTIME)?;
        let max_domain = u16::from_le_bytes(reader.bytes(info, 2)?.as_slice().try_into()?) as i32;
        ensure!(
            domain_id <= max_domain,
            "Multiplayer class has no domain vtable"
        );
        let vtable = reader.ptr(info + layout::RUNTIME_VTABLES + domain_id as usize * 8)?;
        ensure!(
            reader.ptr(vtable)? == self.platform
                && reader.byte(vtable + layout::VTABLE_FLAGS)? & 4 != 0,
            "Multiplayer static data is not ready"
        );
        let slots = reader.i32(self.platform + layout::CLASS_VTABLE_SIZE)?;
        ensure!((0..=65536).contains(&slots), "Invalid Mono vtable size");
        let statics = reader.ptr(vtable + layout::VTABLE_STATIC_SLOTS + slots as usize * 8)?;
        let instance = reader.ptr(statics + self.instance_offset)?;
        if instance == 0 {
            bail!("No multiplayer session. Start or load a multiplayer match first");
        }
        ensure!(
            reader.class(instance)? == self.platform,
            "Multiplayer singleton changed"
        );
        let list = reader.ptr(instance + self.members_offset)?;
        if list == 0 {
            bail!("No multiplayer player list. Start or load a multiplayer match first");
        }
        let list_class = reader.class(list)?;
        let (namespace, name) = reader.class_name(list_class)?;
        ensure!(
            namespace == "System.Collections.Generic" && name == "List`1",
            "Unexpected multiplayer list type"
        );
        let fields = reader.fields(list_class)?;
        let items_offset = required(&fields, "_items")?;
        let size_offset = required(&fields, "_size")?;
        let version_offset = required(&fields, "_version")?;
        let version = reader.i32(list + version_offset)?;
        let count = reader.i32(list + size_offset)?;
        ensure!(
            (0..=64).contains(&count),
            "Invalid multiplayer member count"
        );
        let array = reader.ptr(list + items_offset)?;
        let capacity = reader.ptr(array + layout::ARRAY_LENGTH)?;
        ensure!(
            count as usize <= capacity && capacity <= 256,
            "Invalid multiplayer member array"
        );
        let items = reader.bytes(array + layout::ARRAY_DATA, count as usize * 8)?;
        let mut players = BTreeMap::new();
        let mut ids = std::collections::BTreeSet::new();
        for item in items.chunks_exact(8) {
            let object = u64::from_le_bytes(item.try_into()?) as usize;
            ensure!(
                reader.class(object)? == self.member,
                "Multiplayer member changed"
            );
            let id = reader.i32(object + self.player_id_offset)?;
            if !(1..=8).contains(&id) {
                continue;
            }
            ensure!(ids.insert(id), "Ambiguous multiplayer player ID");
            let ai = reader.byte(object + self.ai_offset)?;
            let colour = reader.i32(object + self.colour_offset)?;
            let name_object = reader.ptr(object + self.player_name_offset)?;
            let name = if ai == 0 {
                reader.string(name_object)?
            } else {
                String::new()
            };
            ensure!(
                reader.i32(object + self.player_id_offset)? == id
                    && reader.ptr(object + self.player_name_offset)? == name_object
                    && reader.byte(object + self.ai_offset)? == ai
                    && reader.i32(object + self.colour_offset)? == colour,
                "Multiplayer name changed during read"
            );
            if ai == 0 {
                players.insert(
                    id as usize,
                    PlayerIdentity {
                        name: (!name.is_empty()).then_some(name),
                        color: (1..=8).contains(&colour).then_some(colour),
                    },
                );
            }
        }
        ensure!(
            reader.i32(list + version_offset)? == version
                && reader.i32(list + size_offset)? == count
                && reader.ptr(list + items_offset)? == array
                && reader.ptr(instance + self.members_offset)? == list
                && reader.ptr(statics + self.instance_offset)? == instance
                && reader.ptr(self.root_global)? == self.root,
            "Multiplayer list changed during read"
        );
        Ok(MultiplayerPlayers { players })
    }
}

fn file_slice(bytes: &[u8], rva: usize, length: usize) -> Result<&[u8]> {
    fn word(bytes: &[u8], offset: usize) -> Result<u16> {
        Ok(u16::from_le_bytes(
            bytes
                .get(offset..offset + 2)
                .context("Truncated Mono file")?
                .try_into()?,
        ))
    }
    fn dword(bytes: &[u8], offset: usize) -> Result<u32> {
        Ok(u32::from_le_bytes(
            bytes
                .get(offset..offset + 4)
                .context("Truncated Mono file")?
                .try_into()?,
        ))
    }
    ensure!(bytes.starts_with(b"MZ"), "Invalid Mono file");
    let pe = dword(bytes, 0x3c)? as usize;
    ensure!(
        bytes.get(pe..pe + 4) == Some(b"PE\0\0"),
        "Invalid Mono PE header"
    );
    let count = word(bytes, pe + 6)? as usize;
    ensure!(count <= 96, "Invalid Mono section count");
    let start = pe + 24 + word(bytes, pe + 20)? as usize;
    for index in 0..count {
        let section = start + index * 40;
        let va = dword(bytes, section + 12)? as usize;
        let size = dword(bytes, section + 16)? as usize;
        let raw = dword(bytes, section + 20)? as usize;
        if rva >= va && rva.checked_add(length).is_some_and(|end| end <= va + size) {
            let at = raw + rva - va;
            return bytes.get(at..at + length).context("Truncated Mono section");
        }
    }
    bail!("Mono accessor has no file-backed section")
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    struct MemoryImage {
        bytes: Vec<u8>,
        changing_version: Option<usize>,
        version_reads: Cell<usize>,
    }
    impl Memory for MemoryImage {
        fn read(&self, address: usize, length: usize) -> Result<Vec<u8>> {
            let at = address
                .checked_sub(0x10000)
                .context("Unmapped fixture address")?;
            let mut bytes = self
                .bytes
                .get(at..at + length)
                .context("Unmapped fixture read")?
                .to_vec();
            if self.changing_version == Some(address) && length == 4 {
                let count = self.version_reads.get();
                self.version_reads.set(count + 1);
                bytes.copy_from_slice(&(count as i32).to_le_bytes());
            }
            Ok(bytes)
        }
    }
    struct Fixture {
        memory: MemoryImage,
        next: usize,
        root_global: usize,
        assemblies_global: usize,
        instance: usize,
        list: usize,
        first_string: usize,
    }
    impl Fixture {
        fn allocate(&mut self, size: usize) -> usize {
            let address = self.next;
            self.next += size.next_multiple_of(16);
            address
        }
        fn put(&mut self, address: usize, bytes: &[u8]) {
            let at = address - 0x10000;
            self.memory.bytes[at..at + bytes.len()].copy_from_slice(bytes);
        }
        fn pointer(&mut self, address: usize, value: usize) {
            self.put(address, &(value as u64).to_le_bytes());
        }
        fn integer(&mut self, address: usize, value: i32) {
            self.put(address, &value.to_le_bytes());
        }
        fn cstring(&mut self, value: &str) -> usize {
            let address = self.allocate(256);
            self.put(address, value.as_bytes());
            address
        }
        fn class(&mut self, namespace: &str, name: &str, fields: &[(&str, usize)]) -> usize {
            let class = self.allocate(0x120);
            self.put(class + 0x1b, &[1]);
            let name = self.cstring(name);
            let namespace = self.cstring(namespace);
            self.pointer(class + 0x48, name);
            self.pointer(class + 0x50, namespace);
            self.integer(class + 0x100, fields.len() as i32);
            let storage = self.allocate(fields.len() * 0x20);
            self.pointer(class + 0x98, storage);
            for (index, (name, offset)) in fields.iter().enumerate() {
                let name = self.cstring(name);
                self.pointer(storage + index * 0x20 + 8, name);
                self.integer(storage + index * 0x20 + 0x18, *offset as i32);
            }
            class
        }
        fn object(&mut self, class: usize) -> usize {
            let object = self.allocate(0x100);
            let vtable = self.allocate(0x60);
            self.pointer(object, vtable);
            self.pointer(vtable, class);
            object
        }
        fn string(&mut self, class: usize, text: &str) -> usize {
            let object = self.object(class);
            let units: Vec<_> = text.encode_utf16().collect();
            self.integer(object + 0x10, units.len() as i32);
            self.put(
                object + 0x14,
                &units
                    .iter()
                    .flat_map(|u| u.to_le_bytes())
                    .collect::<Vec<_>>(),
            );
            object
        }
        fn new() -> Self {
            let mut f = Self {
                memory: MemoryImage {
                    bytes: vec![0; 0x40000],
                    changing_version: None,
                    version_reads: Cell::new(0),
                },
                next: 0x11000,
                root_global: 0x10000,
                assemblies_global: 0x10008,
                instance: 0,
                list: 0,
                first_string: 0,
            };
            let root = f.allocate(0x100);
            f.pointer(f.root_global, root);
            let assembly = f.allocate(0x100);
            let image = f.allocate(0x600);
            let image_name = f.cstring("Assembly-CSharp");
            f.pointer(image + 0x30, image_name);
            f.pointer(assembly + 0x60, image);
            let node = f.allocate(16);
            f.pointer(node, assembly);
            f.pointer(f.assemblies_global, node);
            let platform = f.class(
                "",
                "Platform_Multiplayer",
                &[("instance", 16), ("gameMembers", 56)],
            );
            let member = f.class(
                "",
                "MPGameMember",
                &[
                    ("playerID", 48),
                    ("playerName", 32),
                    ("colourID", 64),
                    ("skirmishAI", 68),
                ],
            );
            f.pointer(platform + 0x40, image);
            f.pointer(member + 0x40, image);
            f.pointer(member + 0x38, platform);
            f.pointer(platform + 0x108, member);
            let buckets = f.allocate(8);
            f.pointer(buckets, platform);
            f.integer(image + 0x4d0 + 0x18, 1);
            f.pointer(image + 0x4d0 + 0x20, buckets);
            let callback = f.allocate(16);
            f.put(callback, &[0x48, 0x8d, 0x81, 0x08, 0x01, 0x00, 0x00, 0xc3]);
            f.pointer(image + 0x4d0 + 0x10, callback);
            let statics = f.allocate(64);
            let vtable = f.allocate(0x60);
            f.pointer(vtable, platform);
            f.put(vtable + 0x30, &[4]);
            f.pointer(vtable + 0x48, statics);
            let info = f.allocate(16);
            f.pointer(info + 8, vtable);
            f.pointer(platform + 0xd0, info);
            f.instance = f.object(platform);
            f.pointer(statics + 16, f.instance);
            let list_class = f.class(
                "System.Collections.Generic",
                "List`1",
                &[("_items", 24), ("_size", 40), ("_version", 44)],
            );
            let definition = f.class(
                "System.Collections.Generic",
                "List`1",
                &[("_items", 0), ("_size", 0), ("_version", 0)],
            );
            let generic = f.allocate(16);
            f.pointer(generic, definition);
            f.pointer(list_class + 0xf0, generic);
            f.put(list_class + 0x1b, &[3]);
            f.list = f.object(list_class);
            f.pointer(f.instance + 56, f.list);
            let array = f.allocate(0x40);
            f.pointer(array + 0x18, 2);
            f.pointer(f.list + 24, array);
            f.integer(f.list + 40, 2);
            let strings = f.class("System", "String", &[]);
            for (index, id, name) in [(0, 4, "Alex 🏹"), (1, 1, "Рустам")] {
                let object = f.object(member);
                let name = f.string(strings, name);
                if index == 0 {
                    f.first_string = name;
                }
                f.integer(object + 48, id);
                f.integer(object + 64, if id == 4 { 2 } else { 7 });
                f.pointer(object + 32, name);
                f.pointer(array + 0x20 + index * 8, object);
            }
            f
        }
        fn runtime(&self) -> Runtime {
            Runtime::discover(&self.memory, self.root_global, self.assemblies_global).unwrap()
        }
    }

    #[test]
    fn reads_runtime_offsets_unicode_and_real_ids_instead_of_list_order() {
        let f = Fixture::new();
        let names = f.runtime().players(&f.memory).unwrap();
        assert_eq!(names.players.keys().copied().collect::<Vec<_>>(), [1, 4]);
        assert_eq!(names.players[&1].name.as_deref(), Some("Рустам"));
        assert_eq!(names.players[&1].color, Some(7));
        assert_eq!(names.players[&4].name.as_deref(), Some("Alex 🏹"));
        assert_eq!(names.players[&4].color, Some(2));
    }
    #[test]
    fn rejects_oversized_lists_and_strings_and_concurrent_list_changes() {
        let mut f = Fixture::new();
        let runtime = f.runtime();
        f.integer(f.list + 40, 1000);
        assert!(runtime.players(&f.memory).is_err());
        f.integer(f.list + 40, 2);
        f.integer(f.first_string + 0x10, 10000);
        assert!(runtime.players(&f.memory).is_err());
        f.integer(f.first_string + 0x10, 0);
        f.memory.changing_version = Some(f.list + 44);
        assert!(runtime.players(&f.memory).is_err());
    }

    #[test]
    fn missing_names_keep_numbered_slots_and_unknown_colors_keep_manual_colors() {
        let mut f = Fixture::new();
        f.integer(f.first_string + 0x10, 0);
        let array = Reader(&f.memory).ptr(f.list + 24).unwrap();
        let member = Reader(&f.memory).ptr(array + 0x20).unwrap();
        f.integer(member + 64, 99);
        let players = f.runtime().players(&f.memory).unwrap();
        let appearance = players.appearance([8; 8]);
        assert_eq!(appearance[3], ("Player 4".into(), 8));
        assert_eq!(appearance[0], ("Рустам".into(), 7));
        assert_eq!(appearance[1], ("Player 2".into(), 8));
    }

    #[test]
    fn ignores_ai_names_and_colors_and_rejects_stale_sessions() {
        let mut f = Fixture::new();
        let runtime = f.runtime();
        let array = Reader(&f.memory).ptr(f.list + 24).unwrap();
        let member = Reader(&f.memory).ptr(array + 0x20).unwrap();
        f.put(member + 68, &[1]);
        // AI strings need not even be valid: they are never decoded.
        f.pointer(member + 32, 1);
        let players = runtime.players(&f.memory).unwrap();
        assert_eq!(players.players.len(), 1);
        assert_eq!(players.appearance([8; 8])[3], ("Player 4".into(), 8));
        f.pointer(f.instance + 56, 0);
        assert!(runtime.players(&f.memory).is_err());
        f.pointer(f.root_global, 0);
        assert!(runtime
            .players(&f.memory)
            .unwrap_err()
            .to_string()
            .contains("domain changed"));
    }

    #[test]
    fn import_snapshot_does_not_change_after_memory_updates() {
        let mut f = Fixture::new();
        let runtime = f.runtime();
        let imported = runtime.players(&f.memory).unwrap().appearance([8; 8]);
        f.integer(f.first_string + 0x10, 0);
        assert_eq!(imported[3].0, "Alex 🏹");
        assert_eq!(
            runtime.players(&f.memory).unwrap().appearance([8; 8])[3].0,
            "Player 4"
        );
    }
}
