use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, AsyncBufReadExt};
use std::fs::{File, OpenOptions};
use std::io::{Write, BufRead, BufReader as StdBufReader};

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
// SİNİR SİSTEMİ (Network ve Asenkron Yapı)
// Normal main yerine tokio::main kullanıyoruz. Bu, asenkron çalışma zamanını başlatır.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::new();
    
    // 1. İşletim sisteminden 6379 portunu (Redis'in meşhur portu) dinlemek için izin istiyoruz.
    let listener = TcpListener::bind("127.0.0.1:6379").await?;
    println!("RustyCache TCP Sunucusu 6379 portunda başlatıldı. İstemciler bekleniyor...");

    loop {
        // 2. Yeni bir bağlantı gelene kadar burada "uyuruz" (await). 
        // Ancak bu sadece bu task'i uyutur, tüm program kilitlenmez!
        let (mut socket, addr) = listener.accept().await?;
        println!("Yeni bağlantı kabul edildi: {}", addr);

        // Her bağlantı için veritabanımızın referansını klonluyoruz
        let store_clone = store.clone();

        // 3. Bağlanan her yeni istemci için hafif bir "Asenkron Görev" (Task) başlatıyoruz.
        // Go'daki "goroutine" mantığının birebir aynısıdır.
        tokio::spawn(async move {
            // ÇÖZÜM 1: split() yerine into_split() kullanıyoruz.
            // Bu sayede reader ve writer, socket'in referansını değil, 
            // bizzat sahipliğini (ownership) alıyor.
            let (reader, mut writer) = socket.into_split();
            let mut reader = BufReader::new(reader);
            let mut line = String::new();

            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => {
                        println!("Bağlantı kapandı: {}", addr);
                        break;
                    }
                    Ok(_) => {
                        let parts: Vec<&str> = line.trim().split_whitespace().collect();
                        if parts.is_empty() { continue; }

                        // ÇÖZÜM 2: Gelen komutu geçici değil, kalıcı bir değişkene almak
                        let command = parts[0].to_uppercase();
                        match command.as_str() {
                            "SET" if parts.len() >= 3 => {
                                let key = parts[1].to_string();
                                let value = parts[2..].join(" ");
                                store_clone.set(key, value);
                                writer.write_all(b"OK\n").await.unwrap();
                            }
                            "GET" if parts.len() == 2 => {
                                let key = parts[1];
                                if let Some(val) = store_clone.get(key) {
                                    // ÇÖZÜM 3: Formatlanmış metni await'ten önce güvene almak
                                    let response = format!("{}\n", val);
                                    writer.write_all(response.as_bytes()).await.unwrap();
                                } else {
                                    writer.write_all(b"(bulunamadi)\n").await.unwrap();
                                }
                            }
                            _ => {
                                writer.write_all(b"HATA: Gecersiz komut (Kullanim: SET key val / GET key)\n").await.unwrap();
                            }
                        }
                    }
                    Err(e) => {
                        println!("Okuma hatası: {}", e);
                        break;
                    }
                }
            }
        });
    }
}