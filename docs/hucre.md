# Hücre

Hücre, GridCraft 0.19.1'in yerel egui arayüzünü ve Rust hesaplama motorunu kullanır. Varsayılan çalışma kitabı biçimi `.xlsx` dosyasıdır.

```powershell
cargo run --release --locked -p gridcraft
```

Windows derlemesinin dosya adı `target/release/gridcraft.exe`, uygulama adı **Hücre**. Örnek bir kitap için komuta `-- --sample sales` ekleyin. Dil menüsünden **Türkçe** seçilebilir. Windows'ta Hücre kendi ayarlarını ve günlüklerini `%APPDATA%\Hucre` altında saklar.

## Masaüstü iş akışları

- Hücre içinde hızlı yazma ve Enter ile onay, tuş olaylarının sırasını korur. Shift+Enter ve Shift+Tab ters yönde ilerler.
- Hücre, satır, sütun ve sayfa sekmesi menülerinde simgeler bulunur. Menü tuşu veya Shift+F10, ızgaradaki seçime uygun menüyü açar. Hücre menüsü düzenleme, kaydırarak ekleme ve silme, sıralama, filtre, sayı biçimi ve dondurma seçeneklerini içerir.
- Tablo başlığındaki filtre açılır listesi yalnızca doğru tablonun seçeneklerini gösterir. Filtreyi uygulamak veya temizlemek için ilgili düğmeyi kullanın.
- Sayfa taşıma ve gösterme adla seçim yapar. Sayfa silme onay ister.
- CSV ve eski Excel biçimleri gibi içe aktarılan kaynaklar, şifresi açılan çalışma kitapları ve uyumluluk uyarısı veren dosyalar ayrı bir XLSX kopyasına kaydedilir. Uyarısız XLSX kitapları mevcut konumuna kaydedilebilir.

## Yol haritası

- 27 Kasım 2026. Koyu temadaki beyaz sayfa üzerinde normal hücre metninin yazı yumuşatması düzeltildi. Sağ tık menüleri ikon, kısayol ve alt menülerle güncellendi.
- 27 Kasım 2026. Hücre girişi, tablo filtreleri, Türkçe sayfa yönetimi ve XLSX kayıt akışı iyileştirildi. Uzun filtre listelerinde yalnızca görünür seçenekler çiziliyor.
- Excel'de aç-kaydet döngüsünün karşılaştırılması ve büyük çalışma kitaplarında bellek ölçümü doğrulama bekliyor.

GridCraft telifi © 2026 ArtCraft Team and the GridCraft contributors. [MIT lisansı](../LICENSE-MIT) · [Üçüncü taraf bildirimleri](../NOTICE)
