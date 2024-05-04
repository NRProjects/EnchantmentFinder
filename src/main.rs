use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Cursor, Write};
use std::path::{Path};
use std::sync::{Arc, mpsc, Mutex};
use std::{io, thread};
use std::sync::mpsc::{Receiver, Sender};
use std::thread::JoinHandle;
use std::time::SystemTime;
use fastanvil::Region;
use fastnbt::from_bytes;
use serde::Deserialize;
use sha2::{Digest, Sha256};


fn main() -> io::Result<()> {
    let max_threads: usize = std::thread::available_parallelism()?.get();

    let start = SystemTime::now();
    let directory_path = Path::new(r"C:\Users\Admin\Documents\Projects\Rust\scratch\src\world\region");

    let entries = std::fs::read_dir(directory_path)?;
    let entries: Vec<_> = entries.collect();

    let total_files = entries.len();

    let hash_dict = Arc::new(Mutex::new(load_hash_dict("hash_dict.json").unwrap_or_default()));

    let (tx, rx): (Sender<usize>, Receiver<usize>) = mpsc::channel();
    let tx_progress = tx.clone();
    thread::spawn(move || {
        let mut processed_count = 0;
        while let Ok(index) = rx.recv() {
            processed_count += 1;
            println!("Processed {}/{}: {}", processed_count, total_files, index);
        }
    });

    let mut handles = vec![];

    for (index, entry) in entries.into_iter().enumerate() {
        let entry = entry?;
        let path = entry.path();
        let hash_dict =Arc::clone(&hash_dict);
        let tx = tx_progress.clone();

        if path.extension().map_or(false, |ext| ext == "mca") {
            if handles.len() >= max_threads {
                let handle: JoinHandle<()> = handles.remove(0);
                handle.join().expect("DEATHCON 3 THREAD PANICKED");
            }

            let handle = thread::spawn(move || {
                let mut file = File::open(path).expect("Failed to open file");
                let mut data = Vec::new();

                file.read_to_end(&mut data).expect("Failed to read file");

                let hash = calculate_file_hash(&data);

                let mut cursor = Cursor::new(data);
                let result = handle_region(&mut cursor);

                let mut dict = hash_dict.lock().unwrap();
                dict.insert(hash, result);

                tx.send(index).expect("Failed to send progress");
            });

            handles.push(handle);
        }
    }


    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    {
        let hash_dict = hash_dict.lock().unwrap();
        save_hash_dict("hash_dict.json", &*hash_dict).expect("Failed to save hash_dict");
    }

    let duration = start.elapsed();
    println!("All files processed in {:?}", duration);

    Ok(())
}

fn calculate_file_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}


fn handle_region(cursor: &mut Cursor<Vec<u8>>) -> bool {
    let mut region = Region::from_stream(cursor).expect("Failed to parse region");

    for x in 0..32 {
        for z in 0..32 {
            if let Ok(Some(chunk)) = region.read_chunk(x, z) {
                if process_chunk(&chunk) {
                    return true;
                }
            }
        }
    }
    return false;
}

fn process_chunk(data: &[u8]) -> bool {
    let chunk: DoogChunk = match from_bytes(data) {
        Ok(chunk) => chunk,
        Err(_) => return false,
    };

    for block_entity in &chunk.block_entities {
        if let BlockEntityData::Container(container) = &block_entity.data {
            if let Some(items) = &container.items {
                for item in items {
                    if handle_item(item) {
                        return true;
                    }
                }
            }
        }
    }

    return false;
}

fn handle_item(item: &Item) -> bool {
    item.tag.as_ref().and_then(|tag| tag.enchantments.as_ref()).map_or(false, |enchantments| {
        enchantments.iter().any(|enchantments| {
            matches!(enchantments.id.as_str(), "minecraft:timber" | "minecraft:vein_miner")
        })
    })
}

fn load_hash_dict(path: &str) -> io::Result<HashMap<String, bool>> {
    let mut file = File::open(path)?;
    let mut buffer = String::new();
    file.read_to_string(&mut buffer)?;
    let hash_dict: HashMap<String, bool> = serde_json::from_str(&buffer)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(hash_dict)
}

fn save_hash_dict(path: &str, hash_dict: &HashMap<String, bool>) -> io::Result<()> {
    let mut file = File::create(path)?;
    let data = serde_json::to_string(hash_dict)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    file.write_all(data.as_bytes())?;
    file.flush()?;
    Ok(())
}

#[derive(Deserialize, Debug)]
pub struct DoogChunk {
    #[serde(rename = "block_entities")]
    pub block_entities: Vec<BlockEntity>
}

#[derive(Debug, Deserialize)]
pub struct BlockEntity {
    pub x: i32,
    pub y: i32,
    pub z: i32,

    #[serde(flatten)]
    pub data: BlockEntityData,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "id")]
pub enum BlockEntityData {
    #[serde(rename = "minecraft:chest", alias = "minecraft:barrel", alias = "minecraft:trapped_chest", alias = "minecraft:shulker_box", alias = "minecraft:red_shulker_box", alias = "minecraft:lime_shulker_box", alias = "minecraft:pink_shulker_box", alias = "minecraft:gray_shulker_box", alias = "minecraft:cyan_shulker_box", alias = "minecraft:blue_shulker_box", alias = "minecraft:white_shulker_box", alias = "minecraft:brown_shulker_box", alias = "minecraft:green_shulker_box", alias = "minecraft:black_shulker_box", alias = "minecraft:orange_shulker_box", alias = "minecraft:yellow_shulker_box", alias = "minecraft:purple_shulker_box", alias = "minecraft:magenta_shulker_box", alias = "minecraft:light_blue_shulker_box", alias = "minecraft:light_gray_shulker_box")]
    Container(BlockEntityContainer),

    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
pub struct BlockEntityContainer {
    #[serde(rename = "Items")]
    pub items: Option<Vec<Item>>
}

#[derive(Deserialize, Debug)]
pub struct Item {
    #[serde(rename = "Count")]
    pub count: i32,

    #[serde(rename = "Slot")]
    pub slot: i32,

    #[serde(default)]
    pub id: String,

    #[serde(rename = "tag")]
    pub tag: Option<ItemTag>,
}

#[derive(Deserialize, Debug)]
pub struct ItemTag {
    #[serde(rename = "BlockEntityTag")]
    pub block_entity_tag: Option<BlockEntityTag>,

    #[serde(rename = "Enchantments", alias = "StoredEnchantments")]
    pub enchantments: Option<Vec<Enchantment>>,
}

#[derive(Deserialize, Debug)]
pub struct Enchantment {
    #[serde(default)]
    pub id: String,

    #[serde(rename = "lvl")]
    pub level: i32
}

#[derive(Deserialize, Debug)]
pub struct BlockEntityTag {
    #[serde(rename = "Items")]
    pub items: Option<Vec<Item>>,
}