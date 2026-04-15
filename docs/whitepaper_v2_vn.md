# HomeChain: Cuộc Cải Cách Giao Thức Gốc EVM
**Phiên bản 2.0.0 (Sovereign Edition)**
**Ngày:** Tháng 4 năm 2026
**Tác giả:** HomeChain Foundation

---

## Tóm tắt
HomeChain (V2) đại diện cho bước tiến hóa quyết định của hệ sinh thái HomeChain—chuyển đổi từ một nguyên mẫu dựa trên Python sang một **blockchain Layer 1 thuần Rust** hiệu suất cao. Bằng cách triển khai công cụ thực thi không cấp phát (zero-allocation), mục tiêu khối ổn định 15 giây (DDA) và khả năng tương thích EVM chính thức, HomeChain đạt được khả năng mở rộng cấp độ công nghiệp trong khi vẫn duy trì tinh thần phi tập trung "Một CPU, Một Phiếu bầu".

## 1. Hệ Tư Tưởng: Rust & EVM
Cuộc cải cách tập trung vào hiệu suất và khả năng tương tác. HomeChain được xây dựng từ đầu để hỗ trợ hệ sinh thái công cụ Ethereum toàn cầu đồng thời tận dụng hiệu suất vượt trội của Rust.

### 1.1 Lợi thế cạnh tranh của Rust
- **Hashing Không Cấp Phát**: Tối đa hóa hiệu suất sử dụng chu kỳ CPU để tăng hiệu quả đào.
- **Xử lý Đồng thời An toàn**: Xử lý các yêu cầu RPC phức tạp mà không làm hỏng trạng thái hệ thống.
- **An toàn Bộ nhớ**: Đảm bảo tính toàn vẹn của sổ cái toàn cầu.

### 1.2 Khả năng tương thích gốc EVM
- **Chain ID**: 4919 (0x1337).
- **Tiêu chuẩn Cốt lõi**: Hỗ trợ đầy đủ cho MetaMask, Hardhat và Foundry.
- **Độ chính xác**: Tuân thủ tiêu chuẩn 18 chữ số thập phân (Wei) để đảm bảo tính minh bạch tài chính tuyệt đối.

## 2. Kiến trúc Kỹ thuật
Kiến trúc Sovereign của HomeChain được vận hành bởi động cơ **Điều chỉnh Độ khó Động (DDA)**:
- **Thuật toán PoW**: SHA256 tối ưu hóa (ưu tiên CPU).
- **Thời gian khối mục tiêu**: **15 Giây**.
- **Ổn định**: Tự động điều chỉnh độ khó theo thời gian thực.

## 3. Kinh tế Token: Sự Khan hiếm Hình học
$HOME là token tiện ích gốc với tổng cung tối đa là **21.000.000.000 (21 Tỷ)**.

### 3.1 Lịch trình Phát hành
HomeChain sử dụng cơ chế **Halving Theo Tỷ lệ Hình học** để đảm bảo giá trị lâu dài:
- **Phần thưởng khởi điểm**: 2.500 HOME mỗi khối.
- **Thời gian Era 1**: 10 Ngày (57.600 khối).
- **Logic mở rộng**: Độ dài của Era sẽ nhân đôi mỗi khi phần thưởng giảm đi một nửa (Suy giảm hình học).

## 4. Công cụ Lưu trữ
Xây dựng trên nền tảng **SQLite 3** tuân thủ ACID, đảm bảo truy cập tốc độ cao, có chỉ mục cho hàng triệu khối và hóa đơn giao dịch với chi phí phần cứng tối thiểu.

## 5. Kết luận
HomeChain là blockchain tối thượng dành cho người dùng. Bằng cách kết hợp tính an toàn của Rust, sự phổ biến của EVM và tính công bằng của PoW, chúng tôi đang xây dựng một mạng lưới thực sự toàn cầu và có chủ quyền.

---
*Giao thức HomeChain - Được xác thực bởi Rust. Được bảo mật bởi chính bạn.*
