use std::collections::HashMap;
use std::process::Command;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader as TokioBufReader, AsyncBufReadExt};
use std::fs::{File, OpenOptions};
use std::io::{Write, BufRead, BufReader as StdBufReader};
use std::env; 
use tokio::sync::broadcast; 

// 1. THE STRUCT 
// We wrap the HashMap in a Mutex (so only one thread can write at a time)
// and an Arc (Atomic Reference Count) so multiple threads can share ownership.
#[derive(Clone)]
pub struct Store {
    db: Arc<Mutex<HashMap<String, String>>>,
}

// 2. THE IMPL BLOCK 
impl Store {
    // 1. HAYATA DÖNÜŞ (Sunucu Başlarken)
    pub fn new() -> Self {
        // En baştan devasa RAM bloğumuzu ayırıyoruz (Optimizasyonumuz duruyor)
        let mut map = HashMap::with_capacity(10000);

        // Diskteki "DNA" dosyamızı okumayı deniyoruz
        if let Ok(file) = File::open("rusty_cache.log") {
            let reader = StdBufReader::new(file);
            
            // Dosyadaki her bir satırı tek tek oku
            for line in reader.lines() {
                if let Ok(content) = line {
                    // Satırı ilk boşluğa göre ikiye böl (Örn: "kullanici ahmet")
                    let parts: Vec<&str> = content.splitn(2, ' ').collect();
                    if parts.len() == 2 {
                        // Okuduğumuz veriyi RAM'deki haritaya yerleştir
                        map.insert(parts[0].to_string(), parts[1].to_string());
                    }
                }
            }
            println!("Geçmiş veriler diskten başarıyla RAM'e yüklendi!");
        } else {
            println!("Geçmiş veri bulunamadı, sıfırdan başlanıyor.");
        }

        Store {
            db: Arc::new(Mutex::new(map)),
        }
    }

    // 2. DİSKE KAZIMA (Yeni Veri Geldiğinde)
    pub fn set(&self, key: String, value: String) {
        let mut map = self.db.lock().unwrap();
        
        // A) Önce veriyi çok hızlı olan RAM'e yaz
        map.insert(key.clone(), value.clone());

        // B) Sonra kalıcı olması için diske (Log dosyasına) yaz
        // OpenOptions: Dosya yoksa yarat (create), varsa içindekini silmeden en sonuna ekle (append)
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open("rusty_cache.log") {
            // Diske yazılacak metni hazırla ve sonuna alt satıra geçme karakteri (\n) koy
            let log_line = format!("{} {}\n", key, value);
            
            // Byte'lara çevirip dosyaya fiziksel olarak yaz (Elektronik devrelere giden sinyal!)
            let _ = file.write_all(log_line.as_bytes());
        }
    }

    // GET metodu aynı kalıyor, çünkü okumayı her zaman saniyede milyonlarca 
    // işlem yapabilen RAM'den yapacağız, yavaş olan diskten değil!
    pub fn get(&self, key: &str) -> Option<String> {
        let map = self.db.lock().unwrap();
        map.get(key).cloned()
    }
}
// Network ve Asenkron Yapı
// Normal main yerine tokio::main kullanıyoruz. Bu, asenkron çalışma zamanını başlatır.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Terminalden rol (leader/follower) ve port bilgilerini alıyoruz
    // Örn: cargo run -- 6379 leader
    let args: Vec<String> = env::args().collect();
    let port = if args.len() > 1 { args[1].clone() } else { "6379".to_string() };
    let role = if args.len() > 2 { args[2].clone() } else { "leader".to_string() };
    
    let address = format!("127.0.0.1:{}", port);
    let store = Store::new();
    
    // 2. RADYO KULESİ: Liderin gelen komutları takipçilere fırlatacağı asenkron yayın kanalı
    let (tx, _rx) = broadcast::channel::<String>(16);

    // 3. EĞER TAKİPÇİYSEK: Başlar başlamaz arka planda Lidere bağlan ve verileri kopyala
    if role == "follower" {
        let leader_addr = if args.len() > 3 { args[3].clone() } else { "127.0.0.1:6379".to_string() };
        println!("Takipçi (Follower) modunda başlatılıyor. Lidere bağlanılıyor: {}...", leader_addr);
        
        let store_bg = store.clone();
        tokio::spawn(async move {
            // Lidere TCP soketi açıyoruz
            if let Ok(mut stream) = tokio::net::TcpStream::connect(&leader_addr).await {
                println!("Lidere başarıyla bağlanıldı! Senkronizasyon (Replication) başlatılıyor...");
                let (reader, mut writer) = stream.into_split();
                
                // Lidere özel şifreyi fırlat: "Beni eşitle!"
                writer.write_all(b"SYNC\n").await.unwrap();
                
                let mut reader = TokioBufReader::new(reader);
                let mut line = String::new();
                
                // Liderden gelen radyo yayınlarını dinle ve kendi motoruna kaydet
                while let Ok(n) = reader.read_line(&mut line).await {
                    if n == 0 { break; } // Lider koptu veya çöktü
                    let parts: Vec<&str> = line.trim().split_whitespace().collect();
                    if parts.len() >= 3 && parts[0].to_uppercase() == "SET" {
                        let key = parts[1].to_string();
                        let value = parts[2..].join(" ");
                        store_bg.set(key, value); // Takipçinin kendi RAM'ine/Diskine yaz!
                    }
                    line.clear();
                }
                println!("Lider ile bağlantı koptu!");
            } else {
                println!("HATA: Lidere bağlanılamadı! Liderin açık olduğundan emin olun.");
            }
        });
    }

    // 4. Ana TCP Dinleyicisi (Hem Lider hem Takipçi istemcileri dinler)
    let listener = TcpListener::bind(&address).await?;
    println!("RustyCache [{}] modunda {} adresinde dinlemeye başladı.", role.to_uppercase(), address);

    loop {
        let (socket, addr) = listener.accept().await?;
        let store_clone = store.clone();
        
        let tx_clone = tx.clone(); // Her bağlantıya radyo vericisinden bir kopya
        let mut rx = tx.subscribe(); // Radyo alıcısı (sadece SYNC diyenler kullanacak)

        // role değişkeninin bu döngü (bağlantı) için bir kopyasını alıyoruz
        let role_clone = role.clone();

        tokio::spawn(async move {
            let (reader, mut writer) = socket.into_split();
            let mut reader = TokioBufReader::new(reader);
            let mut line = String::new();

            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => break,
                    Ok(_) => {
                        let parts: Vec<&str> = line.trim().split_whitespace().collect();
                        if parts.is_empty() { continue; }

                        let command = parts[0].to_uppercase();
                        match command.as_str() {
                            "SET" if parts.len() >= 3 => {
                                let key = parts[1].to_string();
                                let value = parts[2..].join(" ");
                                
                                // 1. Önce veriyi kendi RAM/Disk motorumuza yazıyoruz
                                store_clone.set(key.clone(), value.clone());
                                writer.write_all(b"OK\n").await.unwrap();
                                
                                // 2. EĞER LİDERSEK: Yeni veriyi radyodan tüm takipçilere anons et!
                                if role_clone == "leader" {
                                    let _ = tx_clone.send(format!("SET {} {}\n", key, value));
                                }
                            }
                            "GET" if parts.len() == 2 => {
                                let key = parts[1];
                                if let Some(val) = store_clone.get(key) {
                                    let response = format!("{}\n", val);
                                    writer.write_all(response.as_bytes()).await.unwrap();
                                } else {
                                    writer.write_all(b"(bulunamadi)\n").await.unwrap();
                                }
                            }
                            "SYNC" => {
                                // Ağdan biri bağlandı ve "SYNC" dedi. O artık bir takipçi!
                                println!("--> YENİ TAKİPÇİ (Replica) AĞA KATILDI: {}", addr);
                                
                                // Bu bağlantıyı özel bir döngüye sokuyoruz. Artık istemci değil, 
                                // o sadece radyo kulesini dinleyen bir dinleyici.
                                loop {
                                    if let Ok(msg) = rx.recv().await {
                                        // Radyodan 'SET' yayını geldiğinde, TCP kablosundan takipçiye fırlat
                                        if writer.write_all(msg.as_bytes()).await.is_err() {
                                            println!("Takipçi {} ağdan düştü.", addr);
                                            break; 
                                        }
                                    }
                                }
                                break; // Döngüyü kır
                            }
                            _ => {
                                writer.write_all(b"HATA: Gecersiz komut\n").await.unwrap();
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });
    }
}
