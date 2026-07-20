# NovaKey

[English](README.md)

Bộ gõ tiếng Việt cho macOS sử dụng kỹ thuật backspace.

Nhanh, nhẹ (228KB), tương thích với trình duyệt, terminal và mọi ứng dụng macOS.

## Tính năng

- **Kiểu gõ Telex** hỗ trợ đầy đủ dấu thanh (s/f/r/x/j/z) và biến đổi nguyên âm (aa/ee/oo/aw/ow/uw/dd)
- **Kỹ thuật backspace** -- sử dụng CGEvent tap thay vì IMKit, hoạt động được trên thanh địa chỉ trình duyệt, Terminal, VS Code, Spotlight và mọi nơi
- **Đặt dấu thông minh** -- theo quy tắc chính tả tiếng Việt hiện đại (ví dụ: `hoang` + `f` đặt dấu trên `a`, không phải `o`)
- **Bảo vệ từ tiếng Anh** -- các biến đổi Telex chỉ áp dụng khi âm tiết hợp lệ về cấu trúc, nên `class`, `know`, `add` giữ nguyên thay vì bị biến thành tiếng Việt
- **Nhấn đúp escape / gõ n+1** -- gõ một phím Telex hai lần sẽ tin tưởng lần nhấn thứ hai vô điều kiện, nên `disst` → `dist`, `noww` → `now`, `corrrection` → `correction`
- **Gõ tiếng Việt nhanh (tùy chọn)** -- một chữ `w` đứng sau phụ âm đầu sẽ thành `ư` (ví dụ: `tw` → `tư`), có hoàn tác thời gian thực nên các từ tiếng Anh lẫn lộn như `huawei` vẫn giữ nguyên
- **Ứng dụng thanh menu** -- chạy dưới dạng biểu tượng trên thanh trạng thái (V/E), không hiện trên Dock
- **Sửa lỗi autocomplete trình duyệt** -- dò vùng chọn của ô nhập đang focus để bù cho gợi ý nội tuyến ở thanh địa chỉ, tránh đếm sai số backspace
- **Tự phục hồi sau sleep/wake** -- tự động khởi động lại event tap sau khi máy ngủ
- **Phím tắt chuyển đổi tùy chỉnh** -- mặc định `Option+Z`; có thể đặt lại trong Cài đặt
- **Chuyển bằng phím Fn (Globe)** -- tùy chọn đồng bộ với nút chuyển ngôn ngữ của hệ thống qua phím Fn/Globe
- **Khởi động cùng đăng nhập** và **âm thanh khi chuyển** (tùy chọn)

## Yêu cầu hệ thống

- macOS 14.0+ (Sonoma trở lên)
- Apple Silicon (arm64)
- Quyền **Input Monitoring** (Cài đặt hệ thống > Quyền riêng tư & Bảo mật > Input Monitoring)
- Quyền **Accessibility** (Cài đặt hệ thống > Quyền riêng tư & Bảo mật > Accessibility)

## Biên dịch

```bash
swift build -c release
```

### Đóng gói thành .app

```bash
./build.sh
```

Script này tự động tạo bundle, sao chép tài nguyên và ký ad-hoc.

### Chạy ứng dụng

```bash
open build/NovaKey.app
```

Lần chạy đầu tiên, macOS sẽ yêu cầu cấp quyền. Cấp cả hai:
1. **Cài đặt hệ thống > Quyền riêng tư & Bảo mật > Input Monitoring** -- bật NovaKey
2. **Cài đặt hệ thống > Quyền riêng tư & Bảo mật > Accessibility** -- bật NovaKey

Thanh menu sẽ hiển thị **V** (chế độ tiếng Việt) hoặc **E** (chế độ tiếng Anh).

## Cách sử dụng

### Bảng gõ Telex

| Bạn gõ | Kết quả | Quy tắc |
|--------|---------|---------|
| `as` | `á` | dấu sắc |
| `af` | `à` | dấu huyền |
| `ar` | `ả` | dấu hỏi |
| `ax` | `ã` | dấu ngã |
| `aj` | `ạ` | dấu nặng |
| `az` | `a` | xóa dấu |
| `aa` | `â` | mũ |
| `ee` | `ê` | mũ |
| `oo` | `ô` | mũ |
| `aw` | `ă` | trăng |
| `ow` | `ơ` | móc |
| `uw` | `ư` | móc |
| `dd` | `đ` | đ ngang |

### Phím tắt

| Phím tắt | Chức năng |
|----------|-----------|
| `Option+Z` | Chuyển đổi chế độ Việt/Anh (mặc định -- có thể đặt lại trong Cài đặt) |

### Cài đặt

Nhấn vào biểu tượng **V/E** trên thanh menu > **Settings** để cấu hình:

**Input Method**
- **Toggle hotkey** -- ghi lại phím tắt riêng để chuyển đổi Việt/Anh
- **Quick Vietnamese** -- tắt mặc định; một chữ `w` sau phụ âm sẽ thành `ư` (ví dụ: `tw` → `tư`), trong khi từ lẫn lộn như `huawei` vẫn giữ nguyên

**General**
- **Launch at login** -- tự động khởi động NovaKey
- **Play sound on switch** -- phát âm thanh khi chuyển chế độ
- **Switch with Fn (Globe) key** -- bật mặc định; đồng bộ với nút chuyển ngôn ngữ của hệ thống

**Compatibility**
- **Fix browser autocomplete** -- bật mặc định, giúp gõ tiếng Việt trên thanh địa chỉ Chrome/Safari
- **Send keys step-by-step** -- tắt mặc định, bật nếu bạn thấy ký tự bị lỗi trên một số ứng dụng

## Kiến trúc

```
Bàn phím → CGEventTap (chặn) → TelexEngine (xử lý) → KeySender (xóa + thay thế) → Ứng dụng
```

### Cấu trúc dự án

```
Sources/NovaKey/
├── App/                    # Điểm khởi chạy, delegate, logging
├── Engine/                 # Engine Telex thuần Swift (không phụ thuộc UI/hệ thống)
│   ├── TelexEngine.swift   # Máy trạng thái chính
│   ├── SyllableBuffer.swift# Theo dõi âm tiết hiện tại
│   ├── TonePlacement.swift # Thuật toán đặt dấu thông minh
│   ├── VietnameseData.swift# Bảng Unicode, ánh xạ Telex
│   ├── SpellingChecker.swift# Kiểm tra âm tiết hợp lệ
│   └── KeyCode.swift       # Mã phím ảo macOS
├── EventTap/               # CGEvent tap + gửi phím tổng hợp
│   ├── EventTapManager.swift   # Vòng đời tap, callback sự kiện
│   ├── KeySender.swift         # Gửi backspace + ký tự Unicode
│   └── EventSourceManager.swift# Phát hiện sự kiện do chính mình tạo
├── UI/                     # Biểu tượng thanh menu + cài đặt SwiftUI
├── Settings/               # Lưu trữ UserDefaults, phím tắt
└── Permissions/            # Kiểm tra quyền Input Monitoring + Accessibility
```

### Cách hoạt động của kỹ thuật Backspace

1. CGEvent tap chặn mọi phím gõ trên toàn hệ thống
2. TelexEngine xử lý phím qua máy trạng thái
3. Nếu có biến đổi Telex (ví dụ: gõ `s` sau `a` → `á`):
   - Phím gốc bị **chặn lại** (callback trả về nil)
   - KeySender gửi **N phím backspace** để xóa ký tự cũ
   - KeySender gửi **ký tự tiếng Việt thay thế** qua `CGEventKeyboardSetUnicodeString`
4. Phát hiện sự kiện do chính mình tạo (qua `CGEventSource.sourceStateID`) để tránh vòng lặp vô hạn

### Tại sao không dùng IMKit?

Input Method Kit của Apple sử dụng "cửa sổ soạn thảo" (marked text) để hiển thị ký tự đang gõ. Nhiều ứng dụng không hỗ trợ tốt:
- Thanh địa chỉ trình duyệt bỏ qua marked text
- Các terminal emulator xử lý không nhất quán
- Ứng dụng Electron thường bị lỗi

Kỹ thuật backspace bỏ qua tất cả vấn đề này bằng cách làm việc trực tiếp ở tầng phím gõ.

## Kiểm thử

```bash
# Biên dịch và chạy test engine (không cần Xcode)
swiftc -o /tmp/novakey_tests \
  Sources/NovaKey/Engine/*.swift \
  Tests/run_tests.swift \
  -framework Carbon \
  -parse-as-library && /tmp/novakey_tests
```

87 test bao gồm:
- Tất cả dấu thanh (sắc, huyền, hỏi, ngã, nặng, xóa dấu)
- Tất cả biến đổi nguyên âm (mũ, trăng, móc)
- Đ ngang, chuỗi kết hợp
- Quy tắc đặt dấu thông minh
- Các thao tác trên bộ đệm âm tiết
- Xử lý phím ngắt từ và phím bổ trợ
- Bảo vệ từ tiếng Anh và phục hồi chính tả
- Nhấn đúp escape / gõ n+1

## Gỡ lỗi

Log được ghi vào `/tmp/novakey.log`:

```bash
tail -f /tmp/novakey.log
```

## Giấy phép

MIT — xem [LICENSE](LICENSE).
