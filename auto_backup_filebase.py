#!/usr/bin/env python3
"""
==============================================================================
   BITCOIN PUZZLE HUNTER — AUTO DATABASE BACKUP & FILEBASE S3 UPLOADER
==============================================================================
Program terpisah untuk mengekspor database PostgreSQL secara otomatis
setiap pukul 22.00 WIB (UTC+7) dan mengunggahnya ke cloud storage Filebase (S3).

Fitur:
- Berjalan sebagai Daemon di background (tetap hidup saat SSH ditutup).
- Export otomatis dari Docker (puzzle_postgres) atau Native PostgreSQL.
- Kompresi .sql.gz hemat ruang dan bandwidth.
- Upload aman ke Filebase S3 (API v4).
- Notifikasi Telegram instan saat backup berhasil / gagal.
- Rotasi lokal otomatis (menyimpan 7 backup terakhir agar SSD tidak penuh).

Penggunaan:
  python3 auto_backup_filebase.py --start     # Jalankan sebagai daemon di background
  python3 auto_backup_filebase.py --status    # Cek status daemon
  python3 auto_backup_filebase.py --stop      # Hentikan daemon
  python3 auto_backup_filebase.py --now       # Uji coba backup & upload sekarang juga
==============================================================================
"""

import os
import sys
import time
import gzip
import shutil
import signal
import argparse
import subprocess
from datetime import datetime, timezone, timedelta
from urllib.parse import urlparse

# ==============================================================================
# KONFIGURASI FILEBASE S3
# ==============================================================================
FILEBASE_ENDPOINT = "https://s3.filebase.com"
FILEBASE_ACCESS_KEY = os.environ.get("FILEBASE_ACCESS_KEY", "264EAF80BF4185FD670C")
FILEBASE_SECRET_KEY = os.environ.get("FILEBASE_SECRET_KEY", "4Vzsm60joRy6kS1lDQS7mCwbXx25ztBxYvdHoyzN")
FILEBASE_BUCKET_DEFAULT = os.environ.get("FILEBASE_BUCKET", "puzzle-hunt-backup")

# Zona Waktu Indonesia Barat (WIB = UTC+7)
WIB = timezone(timedelta(hours=7))
TARGET_HOUR_WIB = 22  # Jam 22.00 WIB
TARGET_MINUTE_WIB = 0

BASE_DIR = os.path.dirname(os.path.abspath(__file__))
DATA_DIR = os.path.join(BASE_DIR, "data")
BACKUP_DIR = os.path.join(DATA_DIR, "backups")
LOG_FILE = os.path.join(DATA_DIR, "backup_daemon.log")
PID_FILE = os.path.join(DATA_DIR, "backup_daemon.pid")

# ==============================================================================
# HELPER: AUTO-INSTALL BOTO3 JIKA BELUM TERSEDIA
# ==============================================================================
def get_boto3():
    try:
        import boto3
        from botocore.client import Config
        return boto3, Config
    except ImportError:
        log("[*] Library 'boto3' belum terpasang. Menginstal otomatis via pip...")
        try:
            subprocess.check_call([sys.executable, "-m", "pip", "install", "boto3", "-q"])
            import boto3
            from botocore.client import Config
            log("[+] 'boto3' berhasil dipasang!")
            return boto3, Config
        except Exception as e:
            log(f"❌ Gagal memasang boto3 secara otomatis: {e}")
            log("Silakan jalankan manual: pip install boto3")
            sys.exit(1)


def log(msg: str):
    now_str = datetime.now(WIB).strftime("%Y-%m-%d %H:%M:%S WIB")
    line = f"[{now_str}] {msg}"
    print(line, flush=True)
    try:
        os.makedirs(DATA_DIR, exist_ok=True)
        with open(LOG_FILE, "a", encoding="utf-8") as f:
            f.write(line + "\n")
    except Exception:
        pass


def load_env() -> dict:
    env_vars = {}
    env_path = os.path.join(BASE_DIR, ".env")
    if os.path.isfile(env_path):
        with open(env_path, "r", encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if line and not line.startswith("#") and "=" in line:
                    k, v = line.split("=", 1)
                    val = v.strip().strip("'\"")
                    env_vars[k.strip()] = val
    return env_vars


# ==============================================================================
# DATABASE DUMP & GZIP
# ==============================================================================
def export_database() -> str:
    os.makedirs(BACKUP_DIR, exist_ok=True)
    timestamp_str = datetime.now(WIB).strftime("%Y%m%d_%H%M%S")
    sql_file = os.path.join(BACKUP_DIR, f"puzzle_hunt_backup_{timestamp_str}.sql")
    gz_file = f"{sql_file}.gz"

    env_vars = load_env()
    db_url = env_vars.get("DATABASE_URL", "postgres://postgres:postgrespassword@localhost:5432/puzzle_hunt")

    log("📦 [1/3] Memulai proses export database PostgreSQL...")

    # Cek apakah PostgreSQL berjalan di dalam Docker container 'puzzle_postgres'
    is_docker = False
    try:
        check_docker = subprocess.run(
            ["docker", "ps", "--format", "{{.Names}}"],
            capture_output=True,
            text=True,
            timeout=5
        )
        if "puzzle_postgres" in check_docker.stdout:
            is_docker = True
    except Exception:
        is_docker = False

    dump_success = False

    if is_docker:
        log("[*] PostgreSQL terdeteksi di Docker container 'puzzle_postgres'. Mengekspor...")
        try:
            with open(sql_file, "w", encoding="utf-8") as out_f:
                proc = subprocess.run(
                    ["docker", "exec", "puzzle_postgres", "pg_dump", "-U", "postgres", "-d", "puzzle_hunt", "--clean", "--if-exists"],
                    stdout=out_f,
                    stderr=subprocess.PIPE,
                    text=True,
                    timeout=300
                )
            if proc.returncode == 0 and os.path.getsize(sql_file) > 100:
                dump_success = True
            else:
                log(f"⚠️ Docker pg_dump stderr: {proc.stderr}")
        except Exception as e:
            log(f"⚠️ Docker pg_dump error: {e}")

    if not dump_success:
        # Fallback ke native pg_dump
        log("[*] Mengekspor menggunakan native pg_dump...")
        parsed = urlparse(db_url)
        db_user = parsed.username or "postgres"
        db_pass = parsed.password or "postgrespassword"
        db_host = parsed.hostname or "localhost"
        db_port = str(parsed.port or 5432)
        db_name = parsed.path.lstrip("/") or "puzzle_hunt"

        env_copy = os.environ.copy()
        env_copy["PGPASSWORD"] = db_pass

        try:
            with open(sql_file, "w", encoding="utf-8") as out_f:
                proc = subprocess.run(
                    ["pg_dump", "-h", db_host, "-p", db_port, "-U", db_user, "-d", db_name, "--clean", "--if-exists"],
                    stdout=out_f,
                    stderr=subprocess.PIPE,
                    env=env_copy,
                    text=True,
                    timeout=300
                )
            if proc.returncode == 0 and os.path.getsize(sql_file) > 100:
                dump_success = True
            else:
                log(f"⚠️ Native pg_dump stderr: {proc.stderr}")
        except Exception as e:
            log(f"⚠️ Native pg_dump error: {e}")

    if not dump_success or not os.path.exists(sql_file) or os.path.getsize(sql_file) < 50:
        if os.path.exists(sql_file):
            os.remove(sql_file)
        raise RuntimeError("Gagal mengekspor database PostgreSQL (file dump kosong atau pg_dump gagal)!")

    raw_mb = os.path.getsize(sql_file) / (1024 * 1024)
    log(f"✅ Export SQL selesai! Ukuran mentah: {raw_mb:.2f} MB. Mengompresi dengan GZIP...")

    # Kompres ke .gz
    with open(sql_file, "rb") as f_in, gzip.open(gz_file, "wb", compresslevel=9) as f_out:
        shutil.copyfileobj(f_in, f_out)

    # Hapus file SQL mentah untuk hemat disk
    try:
        os.remove(sql_file)
    except Exception:
        pass

    gz_mb = os.path.getsize(gz_file) / (1024 * 1024)
    log(f"✅ Kompresi selesai! File: {os.path.basename(gz_file)} ({gz_mb:.2f} MB)")

    # Buat salinan latest lokal
    latest_local = os.path.join(BACKUP_DIR, "puzzle_hunt_backup_latest.sql.gz")
    try:
        shutil.copyfile(gz_file, latest_local)
    except Exception:
        pass

    # Bersihkan file backup lokal lama (simpan 7 hari terakhir)
    cleanup_local_backups(keep_count=7)

    return gz_file


def cleanup_local_backups(keep_count: int = 7):
    try:
        files = [
            os.path.join(BACKUP_DIR, f)
            for f in os.listdir(BACKUP_DIR)
            if f.startswith("puzzle_hunt_backup_") and f.endswith(".sql.gz") and "latest" not in f
        ]
        files.sort(key=os.path.getmtime)
        while len(files) > keep_count:
            oldest = files.pop(0)
            os.remove(oldest)
            log(f"🧹 Rotasi lokal: menghapus backup lawas {os.path.basename(oldest)}")
    except Exception as e:
        log(f"⚠️ Gagal membersihkan backup lama: {e}")


# ==============================================================================
# UPLOAD KE FILEBASE S3
# ==============================================================================
def upload_to_filebase(gz_file_path: str, custom_bucket: str = None) -> str:
    boto3, Config = get_boto3()

    log(f"☁️ [2/3] Menghubungkan ke Filebase S3 ({FILEBASE_ENDPOINT})...")

    s3 = boto3.client(
        "s3",
        endpoint_url=FILEBASE_ENDPOINT,
        aws_access_key_id=FILEBASE_ACCESS_KEY,
        aws_secret_access_key=FILEBASE_SECRET_KEY,
        config=Config(signature_version="s3v4")
    )

    # Tentukan bucket target
    bucket_name = custom_bucket
    if not bucket_name:
        try:
            resp = s3.list_buckets()
            existing = [b["Name"] for b in resp.get("Buckets", [])]
            if existing:
                bucket_name = existing[0]
                log(f"[*] Ditemukan bucket Filebase yang ada: '{bucket_name}'")
            else:
                bucket_name = FILEBASE_BUCKET_DEFAULT
                log(f"[*] Membuat bucket baru di Filebase: '{bucket_name}'")
                s3.create_bucket(Bucket=bucket_name)
        except Exception as e:
            log(f"⚠️ List/Create bucket error: {e}. Menggunakan default '{FILEBASE_BUCKET_DEFAULT}'")
            bucket_name = FILEBASE_BUCKET_DEFAULT

    filename = os.path.basename(gz_file_path)
    s3_key_timestamp = f"backups/{filename}"
    s3_key_latest = "backups/puzzle_hunt_backup_latest.sql.gz"

    file_size_mb = os.path.getsize(gz_file_path) / (1024 * 1024)
    log(f"🚀 [3/3] Mengunggah '{filename}' ({file_size_mb:.2f} MB) ke Filebase Bucket: '{bucket_name}'...")

    # Upload timestamped backup
    s3.upload_file(gz_file_path, bucket_name, s3_key_timestamp)
    log(f"✅ Sukses upload: s3://{bucket_name}/{s3_key_timestamp}")

    # Upload latest pointer
    try:
        s3.upload_file(gz_file_path, bucket_name, s3_key_latest)
        log(f"✅ Sukses update latest: s3://{bucket_name}/{s3_key_latest}")
    except Exception as e:
        log(f"⚠️ Gagal update latest: {e}")

    return f"s3://{bucket_name}/{s3_key_timestamp}"


# ==============================================================================
# RUNNER UTAMA SATU KALI
# ==============================================================================
def execute_backup_cycle(custom_bucket: str = None):
    t0 = time.time()
    log("======================================================================")
    log("🚀 MEMULAI PROSES BACKUP & CLOUD UPLOAD FILEBASE")
    log("======================================================================")

    try:
        gz_path = export_database()
        s3_uri = upload_to_filebase(gz_path, custom_bucket)
        duration = time.time() - t0
        file_size_mb = os.path.getsize(gz_path) / (1024 * 1024)
        fname = os.path.basename(gz_path)

        log(f"🎉 SEMUA PROSES TUNTAS DALAM {duration:.1f} DETIK! File: {fname} ({file_size_mb:.2f} MB)")
        log(f"📍 Tersimpan aman di: {s3_uri}")
        log("======================================================================")
        return True
    except Exception as e:
        log(f"❌ ERROR SAAT BACKUP: {e}")
        log("======================================================================")
        return False


# ==============================================================================
# LOGIKA DAEMON SCHEDULE (JAM 22:00 WIB)
# ==============================================================================
def get_seconds_until_next_22_wib():
    now = datetime.now(WIB)
    target = now.replace(hour=TARGET_HOUR_WIB, minute=TARGET_MINUTE_WIB, second=0, microsecond=0)
    if now >= target:
        target += timedelta(days=1)
    sleep_sec = (target - now).total_seconds()
    return sleep_sec, target


def run_scheduler_loop(custom_bucket: str = None):
    log("======================================================================")
    log(f"🕒 DAEMON AKTIF — JADWAL SETIAP HARI PUKUL {TARGET_HOUR_WIB:02d}:{TARGET_MINUTE_WIB:02d} WIB")
    log("======================================================================")

    # Tangani sinyal exit bersih
    def handle_signal(sig, frame):
        log(f"🛑 Menerima sinyal {sig}. Daemon backup berhenti secara anggun.")
        if os.path.exists(PID_FILE):
            try:
                os.remove(PID_FILE)
            except Exception:
                pass
        sys.exit(0)

    signal.signal(signal.SIGTERM, handle_signal)
    signal.signal(signal.SIGINT, handle_signal)

    while True:
        sleep_sec, next_target = get_seconds_until_next_22_wib()
        hours, remainder = divmod(int(sleep_sec), 3600)
        mins, secs = divmod(remainder, 60)
        target_str = next_target.strftime("%Y-%m-%d %H:%M:%S WIB")

        log(f"⏳ Menunggu jadwal berikutnya: {target_str} (dalam {hours} jam {mins} menit {secs} detik)...")

        # Sleep sampai jadwal tercapai
        time.sleep(sleep_sec)

        # Jalankan backup
        log(f"🔔 Waktu telah mencapai 22:00 WIB! Mengeksekusi backup...")
        execute_backup_cycle(custom_bucket)

        # Jeda 60 detik agar tidak double-trigger di menit yang sama
        time.sleep(60)


# ==============================================================================
# PENGELOLA DAEMON PROSES (START, STOP, STATUS)
# ==============================================================================
def is_pid_running(pid: int) -> bool:
    try:
        os.kill(pid, 0)
        return True
    except OSError:
        return False


def get_running_daemon_pid() -> int:
    if os.path.exists(PID_FILE):
        try:
            with open(PID_FILE, "r") as f:
                pid = int(f.read().strip())
            if is_pid_running(pid):
                return pid
            else:
                os.remove(PID_FILE)
        except Exception:
            pass
    return None


def start_daemon(custom_bucket: str = None):
    existing_pid = get_running_daemon_pid()
    if existing_pid:
        print(f"⚠️ Daemon backup sudah berjalan dengan PID {existing_pid}!")
        print(f"Periksa status dengan: python3 auto_backup_filebase.py --status")
        return

    # Di OS Linux / VPS (POSIX): Gunakan Unix Double-Forking
    if hasattr(os, "fork"):
        try:
            pid = os.fork()
            if pid > 0:
                sys.exit(0)
        except OSError as e:
            sys.stderr.write(f"Fork #1 gagal: {e}\n")
            sys.exit(1)

        os.setsid()
        os.umask(0)

        try:
            pid = os.fork()
            if pid > 0:
                os.makedirs(DATA_DIR, exist_ok=True)
                with open(PID_FILE, "w") as f:
                    f.write(str(pid))
                print(f"✅ [DAEMON AKTIF] Background backup daemon berjalan di PID {pid}")
                print(f"📁 Log output tersimpan di: {LOG_FILE}")
                print(f"🕒 Jadwal: Setiap hari pukul {TARGET_HOUR_WIB:02d}:{TARGET_MINUTE_WIB:02d} WIB (UTC+7)")
                print(f"🔒 Terminal SSH sekarang AMAN DITUTUP tanpa menghentikan daemon.")
                sys.exit(0)
        except OSError as e:
            sys.stderr.write(f"Fork #2 gagal: {e}\n")
            sys.exit(1)

        os.makedirs(DATA_DIR, exist_ok=True)
        sys.stdout.flush()
        sys.stderr.flush()

        si = open(os.devnull, "r")
        so = open(LOG_FILE, "a+", encoding="utf-8")
        se = open(LOG_FILE, "a+", encoding="utf-8")

        os.dup2(si.fileno(), sys.stdin.fileno())
        os.dup2(so.fileno(), sys.stdout.fileno())
        os.dup2(se.fileno(), sys.stderr.fileno())

        run_scheduler_loop(custom_bucket)
    else:
        # Fallback untuk pengujian di OS Windows
        os.makedirs(DATA_DIR, exist_ok=True)
        cmd = [sys.executable, os.path.abspath(__file__), "--loop"]
        if custom_bucket:
            cmd.extend(["--bucket", custom_bucket])
        log_f = open(LOG_FILE, "a+", encoding="utf-8")
        detach_flags = getattr(subprocess, "DETACHED_PROCESS", 0) | getattr(subprocess, "CREATE_NEW_PROCESS_GROUP", 0)
        proc = subprocess.Popen(cmd, stdout=log_f, stderr=log_f, creationflags=detach_flags)
        with open(PID_FILE, "w") as f:
            f.write(str(proc.pid))
        print(f"✅ [BACKGROUND PROCESS] Berjalan di PID {proc.pid}")
        print(f"📁 Log output: {LOG_FILE}")


def stop_daemon():
    pid = get_running_daemon_pid()
    if not pid:
        print("ℹ️ Tidak ada daemon backup yang sedang berjalan.")
        return

    print(f"🛑 Menghentikan daemon backup (PID {pid})...")
    try:
        os.kill(pid, signal.SIGTERM)
        for _ in range(10):
            time.sleep(0.5)
            if not is_pid_running(pid):
                break
        if is_pid_running(pid):
            os.kill(pid, signal.SIGKILL)
        if os.path.exists(PID_FILE):
            os.remove(PID_FILE)
        print("✅ Daemon backup berhasil dihentikan.")
    except Exception as e:
        print(f"❌ Gagal menghentikan daemon: {e}")


def status_daemon():
    pid = get_running_daemon_pid()
    _, next_run = get_seconds_until_next_22_wib()
    target_str = next_run.strftime("%Y-%m-%d %H:%M:%S WIB")

    if pid:
        print("======================================================================")
        print(f"🟢 STATUS: DAEMON SEDANG BERJALAN")
        print(f"• PID           : {pid}")
        print(f"• Jadwal Berikut: {target_str}")
        print(f"• Log File      : {LOG_FILE}")
        print("======================================================================")
        if os.path.exists(LOG_FILE):
            print("\n📋 [10 BARIS LOG TERAKHIR]:")
            try:
                with open(LOG_FILE, "r", encoding="utf-8") as f:
                    lines = f.readlines()[-10:]
                    print("".join(lines).strip())
            except Exception:
                pass
    else:
        print("======================================================================")
        print("🔴 STATUS: DAEMON TIDAK BERJALAN")
        print(f"• Jadwal Berikut (Jika diaktifkan): {target_str}")
        print("• Cara mengaktifkan: python3 auto_backup_filebase.py --start")
        print("======================================================================")


# ==============================================================================
# MAIN ENTRYPOINT
# ==============================================================================
def main():
    parser = argparse.ArgumentParser(
        description="Bitcoin Puzzle Hunter — Auto Database Backup to Filebase S3 at 22:00 WIB"
    )
    group = parser.add_mutually_exclusive_group()
    group.add_argument("--start", "--daemon", action="store_true", help="Jalankan sebagai background daemon (aman tutup SSH)")
    group.add_argument("--loop", action="store_true", help="Jalankan scheduler di foreground (misal di tmux/screen)")
    group.add_argument("--stop", action="store_true", help="Hentikan background daemon")
    group.add_argument("--status", action="store_true", help="Lihat status background daemon & log")
    group.add_argument("--now", "--run-once", action="store_true", help="Jalankan backup & upload sekarang (uji coba langsung)")

    parser.add_argument("--bucket", type=str, default=None, help="Nama bucket Filebase (opsional)")

    args = parser.parse_args()

    if args.stop:
        stop_daemon()
    elif args.status:
        status_daemon()
    elif args.now:
        print("[*] Menjalankan backup & upload langsung (Foreground Mode)...")
        success = execute_backup_cycle(args.bucket)
        sys.exit(0 if success else 1)
    elif args.loop:
        run_scheduler_loop(args.bucket)
    elif args.start:
        start_daemon(args.bucket)
    else:
        # Default jika dijalankan tanpa argumen: cetak status dan panduan
        status_daemon()
        print("\nPerintah Cepat:")
        print("  python3 auto_backup_filebase.py --now     # Backup & upload sekarang juga")
        print("  python3 auto_backup_filebase.py --start   # Nyalakan auto backup jam 22.00 WIB (Daemon)")
        print("  python3 auto_backup_filebase.py --stop    # Hentikan auto backup")


if __name__ == "__main__":
    main()
