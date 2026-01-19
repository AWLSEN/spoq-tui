#!/usr/bin/env python3
"""
Universal Browser Cookie Exporter
Auto-discovers and exports cookies from all Chromium-based browsers and Safari.

Supports: macOS, Windows, Linux

Usage: python3 export_all_cookies.py [output_file.csv]

Requires: pip install pycryptodome
"""

import sqlite3
import os
import sys
import shutil
import subprocess
import struct
import platform
import glob
import json
from hashlib import pbkdf2_hmac
from datetime import datetime, timedelta
import csv
import base64

# Check for pycryptodome
try:
    from Crypto.Cipher import AES
except ImportError:
    print("Installing pycryptodome...")
    subprocess.run([sys.executable, "-m", "pip", "install", "pycryptodome", "-q"])
    from Crypto.Cipher import AES


SYSTEM = platform.system()


def get_browser_base_paths():
    """Get base paths where browsers store data for each OS"""
    if SYSTEM == 'Darwin':
        return [os.path.expanduser("~/Library/Application Support")]
    elif SYSTEM == 'Windows':
        return [os.path.expandvars(r"%LOCALAPPDATA%"), os.path.expandvars(r"%APPDATA%")]
    elif SYSTEM == 'Linux':
        return [os.path.expanduser("~/.config"), os.path.expanduser("~/.var/app"), os.path.expanduser("~/snap")]
    return []


KNOWN_CHROMIUM_BROWSERS = {
    'Google/Chrome': 'Chrome', 'Google/Chrome Beta': 'Chrome Beta', 'Google/Chrome Canary': 'Chrome Canary',
    'BraveSoftware/Brave-Browser': 'Brave', 'BraveSoftware/Brave-Browser-Beta': 'Brave Beta',
    'Microsoft Edge': 'Edge', 'Microsoft Edge Beta': 'Edge Beta',
    'Arc/User Data': 'Arc', 'com.operasoftware.Opera': 'Opera', 'Opera Software/Opera Stable': 'Opera',
    'Opera Software/Opera GX Stable': 'Opera GX', 'Vivaldi': 'Vivaldi', 'Chromium': 'Chromium',
    'Coccoc/Browser': 'Coccoc', 'Yandex/YandexBrowser': 'Yandex', 'Sidekick': 'Sidekick',
    'Comet': 'Comet', 'Orion': 'Orion', 'Wavebox': 'Wavebox',
    # Windows
    'Google\\Chrome': 'Chrome', 'BraveSoftware\\Brave-Browser': 'Brave', 'Microsoft\\Edge': 'Edge',
    # Linux
    'google-chrome': 'Chrome', 'brave-browser': 'Brave', 'microsoft-edge': 'Edge', 'chromium': 'Chromium',
}


def discover_chromium_browsers():
    """Auto-discover all Chromium-based browsers"""
    browsers = []
    base_paths = get_browser_base_paths()
    
    for base_path in base_paths:
        if not os.path.exists(base_path):
            continue
        
        for browser_subpath, browser_name in KNOWN_CHROMIUM_BROWSERS.items():
            for cookie_subpath in ["Default/Cookies", "Default/Network/Cookies", "Cookies"]:
                cookie_path = os.path.join(base_path, browser_subpath, cookie_subpath)
                if os.path.exists(cookie_path):
                    browsers.append({
                        'name': browser_name,
                        'cookie_path': cookie_path,
                        'base_path': os.path.join(base_path, browser_subpath),
                    })
                    break
    
    return browsers


def get_keychain_password(service, account):
    """Get password from macOS Keychain"""
    try:
        result = subprocess.run(
            ['security', 'find-generic-password', '-s', service, '-a', account, '-w'],
            capture_output=True, text=True, timeout=5
        )
        if result.returncode == 0:
            return result.stdout.strip()
    except:
        pass
    return None


def get_encryption_key(browser_info):
    """Get the encryption key for a browser"""
    if SYSTEM == 'Darwin':
        return get_macos_key(browser_info)
    elif SYSTEM == 'Windows':
        return get_windows_key(browser_info)
    elif SYSTEM == 'Linux':
        return get_linux_key(browser_info)
    return None


def get_macos_key(browser_info):
    """Get encryption key from macOS Keychain"""
    browser_name = browser_info['name']
    
    keychain_map = {
        'Chrome': ('Chrome Safe Storage', 'Chrome'),
        'Chrome Beta': ('Chrome Safe Storage', 'Chrome'),
        'Chrome Canary': ('Chrome Safe Storage', 'Chrome'),
        'Brave': ('Brave Safe Storage', 'Brave'),
        'Brave Beta': ('Brave Safe Storage', 'Brave'),
        'Edge': ('Microsoft Edge Safe Storage', 'Microsoft Edge'),
        'Edge Beta': ('Microsoft Edge Safe Storage', 'Microsoft Edge'),
        'Arc': ('Arc Safe Storage', 'Arc'),
        'Opera': ('Opera Safe Storage', 'Opera'),
        'Opera GX': ('Opera Safe Storage', 'Opera'),
        'Vivaldi': ('Vivaldi Safe Storage', 'Vivaldi'),
        'Chromium': ('Chromium Safe Storage', 'Chromium'),
        'Comet': ('Comet Safe Storage', 'Comet'),
        'Sidekick': ('Sidekick Safe Storage', 'Sidekick'),
        'Yandex': ('Yandex Safe Storage', 'Yandex'),
        'Wavebox': ('Wavebox Safe Storage', 'Wavebox'),
        'Orion': ('Orion Safe Storage', 'Orion'),
    }
    
    if browser_name in keychain_map:
        service, account = keychain_map[browser_name]
        password = get_keychain_password(service, account)
        if password:
            return pbkdf2_hmac('sha1', password.encode(), b'saltysalt', 1003, dklen=16)
    
    # Try generic pattern
    for service, account in [(f"{browser_name} Safe Storage", browser_name)]:
        password = get_keychain_password(service, account)
        if password:
            return pbkdf2_hmac('sha1', password.encode(), b'saltysalt', 1003, dklen=16)
    
    return None


def get_windows_key(browser_info):
    """Get encryption key from Windows DPAPI"""
    try:
        import win32crypt
    except ImportError:
        return None
    
    local_state_path = os.path.join(browser_info['base_path'], 'Local State')
    if not os.path.exists(local_state_path):
        return None
    
    try:
        with open(local_state_path, 'r', encoding='utf-8') as f:
            local_state = json.load(f)
        encrypted_key = base64.b64decode(local_state['os_crypt']['encrypted_key'])[5:]
        return win32crypt.CryptUnprotectData(encrypted_key, None, None, None, 0)[1]
    except:
        return None


def get_linux_key(browser_info):
    """Get encryption key for Linux"""
    try:
        import secretstorage
        connection = secretstorage.dbus_init()
        collection = secretstorage.get_default_collection(connection)
        for item in collection.get_all_items():
            if browser_info['name'].lower() in item.get_label().lower():
                password = item.get_secret().decode()
                return pbkdf2_hmac('sha1', password.encode(), b'saltysalt', 1, dklen=16)
    except:
        pass
    return pbkdf2_hmac('sha1', b'peanuts', b'saltysalt', 1, dklen=16)


def decrypt_cookie_value(encrypted_value, key):
    """Decrypt a Chromium encrypted cookie value"""
    if not encrypted_value or not key:
        return ""
    
    try:
        # v10 - macOS AES-CBC with 32-byte prefix in decrypted data
        if encrypted_value[:3] == b'v10':
            iv = b' ' * 16
            cipher = AES.new(key, AES.MODE_CBC, iv)
            decrypted = cipher.decrypt(encrypted_value[3:])
            
            # Remove PKCS7 padding
            padding_len = decrypted[-1]
            if padding_len < 16:
                decrypted = decrypted[:-padding_len]
            
            # Skip 32-byte prefix (Chrome adds authentication/hash data)
            if len(decrypted) > 32:
                decrypted = decrypted[32:]
            
            return decrypted.decode('utf-8', errors='replace')
        
        # v11 - Windows AES-GCM
        elif encrypted_value[:3] == b'v11':
            nonce = encrypted_value[3:15]
            ciphertext = encrypted_value[15:-16]
            tag = encrypted_value[-16:]
            cipher = AES.new(key, AES.MODE_GCM, nonce=nonce)
            return cipher.decrypt_and_verify(ciphertext, tag).decode('utf-8', errors='replace')
        
        # No prefix - try DPAPI on Windows or return as-is
        else:
            if SYSTEM == 'Windows':
                try:
                    import win32crypt
                    return win32crypt.CryptUnprotectData(encrypted_value, None, None, None, 0)[1].decode('utf-8')
                except:
                    pass
            return encrypted_value.decode('utf-8', errors='replace')
    
    except Exception as e:
        return ""


def chrome_timestamp_to_datetime(timestamp):
    """Convert Chrome timestamp to datetime string"""
    if timestamp == 0:
        return "Session"
    try:
        dt = datetime(1601, 1, 1) + timedelta(microseconds=timestamp)
        return dt.strftime('%Y-%m-%d %H:%M:%S')
    except:
        return "Unknown"


def export_chromium_cookies(browser_info):
    """Export cookies from a Chromium-based browser"""
    cookies = []
    cookie_path = browser_info['cookie_path']
    
    if not os.path.exists(cookie_path):
        return cookies
    
    key = get_encryption_key(browser_info)
    if not key:
        print(f"  {browser_info['name']}: Could not get encryption key")
        return cookies
    
    temp_db = f"/tmp/{browser_info['name'].replace(' ', '_')}_cookies_export.db"
    if SYSTEM == 'Windows':
        temp_db = os.path.join(os.environ['TEMP'], f"{browser_info['name'].replace(' ', '_')}_cookies.db")
    
    try:
        shutil.copy2(cookie_path, temp_db)
    except Exception as e:
        print(f"  {browser_info['name']}: Could not copy database (browser might be running)")
        return cookies
    
    try:
        conn = sqlite3.connect(temp_db)
        conn.text_factory = bytes
        cursor = conn.cursor()
        
        try:
            cursor.execute("""
                SELECT host_key, name, encrypted_value, path, expires_utc, 
                       is_secure, is_httponly, samesite
                FROM cookies
            """)
        except:
            cursor.execute("""
                SELECT host_key, name, encrypted_value, path, expires_utc, 
                       is_secure, is_httponly, 0
                FROM cookies
            """)
        
        for row in cursor.fetchall():
            host, name, enc_value, path, expires, secure, httponly, samesite = row
            
            decrypted = decrypt_cookie_value(enc_value, key)
            
            cookies.append({
                'browser': browser_info['name'],
                'domain': host.decode('utf-8', errors='replace') if isinstance(host, bytes) else host,
                'name': name.decode('utf-8', errors='replace') if isinstance(name, bytes) else name,
                'value': decrypted,
                'path': path.decode('utf-8', errors='replace') if isinstance(path, bytes) else path,
                'expires': chrome_timestamp_to_datetime(expires),
                'secure': bool(secure),
                'httponly': bool(httponly),
                'samesite': samesite if samesite else 0
            })
        
        conn.close()
    except Exception as e:
        print(f"  {browser_info['name']}: Error - {e}")
    finally:
        try:
            os.remove(temp_db)
        except:
            pass
    
    return cookies


def export_safari_cookies():
    """Export Safari cookies (macOS only)"""
    if SYSTEM != 'Darwin':
        return []
    
    cookies = []
    cookie_file = os.path.expanduser("~/Library/Cookies/Cookies.binarycookies")
    temp_file = "/tmp/safari_cookies_export.bin"
    
    try:
        shutil.copy2(cookie_file, temp_file)
    except PermissionError:
        print("  Safari: Requires Full Disk Access (System Settings → Privacy & Security)")
        return cookies
    except Exception as e:
        print(f"  Safari: {e}")
        return cookies
    
    try:
        with open(temp_file, 'rb') as f:
            if f.read(4) != b'cook':
                return cookies
            num_pages = struct.unpack('>I', f.read(4))[0]
            page_sizes = [struct.unpack('>I', f.read(4))[0] for _ in range(num_pages)]
            for size in page_sizes:
                cookies.extend(parse_safari_page(f.read(size)))
    except Exception as e:
        print(f"  Safari: Error - {e}")
    finally:
        try:
            os.remove(temp_file)
        except:
            pass
    
    return cookies


def parse_safari_page(data):
    """Parse a Safari cookie page"""
    cookies = []
    if data[:4] != b'\x00\x00\x01\x00':
        return cookies
    
    num = struct.unpack('<I', data[4:8])[0]
    offsets = [struct.unpack('<I', data[8+i*4:12+i*4])[0] for i in range(num)]
    
    for off in offsets:
        try:
            d = data[off:]
            flags = struct.unpack('<I', d[8:12])[0]
            
            def read_str(o):
                end = d.find(b'\x00', o)
                return d[o:end].decode('utf-8', errors='replace')
            
            url_off = struct.unpack('<I', d[16:20])[0]
            name_off = struct.unpack('<I', d[20:24])[0]
            path_off = struct.unpack('<I', d[24:28])[0]
            val_off = struct.unpack('<I', d[28:32])[0]
            exp = struct.unpack('<d', d[40:48])[0]
            
            exp_str = (datetime(2001,1,1) + timedelta(seconds=exp)).strftime('%Y-%m-%d %H:%M:%S') if exp > 0 else "Session"
            
            cookies.append({
                'browser': 'Safari',
                'domain': read_str(url_off),
                'name': read_str(name_off),
                'value': read_str(val_off),
                'path': read_str(path_off),
                'expires': exp_str,
                'secure': bool(flags & 0x1),
                'httponly': bool(flags & 0x4),
                'samesite': 0
            })
        except:
            continue
    
    return cookies


def export_firefox_cookies():
    """Export Firefox cookies"""
    cookies = []
    
    if SYSTEM == 'Darwin':
        profile_base = os.path.expanduser("~/Library/Application Support/Firefox/Profiles")
    elif SYSTEM == 'Windows':
        profile_base = os.path.expandvars(r"%APPDATA%\Mozilla\Firefox\Profiles")
    else:
        profile_base = os.path.expanduser("~/.mozilla/firefox")
    
    if not os.path.exists(profile_base):
        return cookies
    
    for profile_dir in glob.glob(os.path.join(profile_base, "*")):
        cookie_file = os.path.join(profile_dir, "cookies.sqlite")
        if not os.path.exists(cookie_file):
            continue
        
        temp_db = "/tmp/firefox_cookies_export.db"
        try:
            shutil.copy2(cookie_file, temp_db)
            conn = sqlite3.connect(temp_db)
            cursor = conn.cursor()
            cursor.execute("SELECT host, name, value, path, expiry, isSecure, isHttpOnly, sameSite FROM moz_cookies")
            
            for row in cursor.fetchall():
                host, name, value, path, expiry, secure, httponly, samesite = row
                exp_str = datetime.fromtimestamp(expiry).strftime('%Y-%m-%d %H:%M:%S') if expiry else "Session"
                cookies.append({
                    'browser': 'Firefox',
                    'domain': host, 'name': name, 'value': value, 'path': path,
                    'expires': exp_str, 'secure': bool(secure), 'httponly': bool(httponly), 'samesite': samesite
                })
            conn.close()
        except:
            pass
        finally:
            try:
                os.remove(temp_db)
            except:
                pass
    
    return cookies


def main():
    output_file = sys.argv[1] if len(sys.argv) > 1 else os.path.expanduser("~/Desktop/all_cookies_export.csv")
    
    print("=" * 60)
    print("Universal Browser Cookie Exporter")
    print(f"System: {SYSTEM}")
    print("=" * 60)
    print()
    
    all_cookies = []
    
    print("Discovering browsers...")
    browsers = discover_chromium_browsers()
    print(f"Found {len(browsers)} Chromium browsers\n")
    
    print("Exporting cookies:")
    for browser in browsers:
        cookies = export_chromium_cookies(browser)
        if cookies:
            print(f"  {browser['name']}: {len(cookies)} cookies")
            all_cookies.extend(cookies)
    
    if SYSTEM == 'Darwin':
        print()
        safari_cookies = export_safari_cookies()
        if safari_cookies:
            print(f"  Safari: {len(safari_cookies)} cookies")
            all_cookies.extend(safari_cookies)
    
    print()
    firefox_cookies = export_firefox_cookies()
    if firefox_cookies:
        print(f"  Firefox: {len(firefox_cookies)} cookies")
        all_cookies.extend(firefox_cookies)
    else:
        print("  Firefox: Not found")
    
    print()
    if all_cookies:
        with open(output_file, 'w', newline='', encoding='utf-8') as f:
            writer = csv.DictWriter(f, fieldnames=['browser', 'domain', 'name', 'value', 'path', 'expires', 'secure', 'httponly', 'samesite'])
            writer.writeheader()
            writer.writerows(all_cookies)
        
        print("=" * 60)
        print(f"TOTAL: {len(all_cookies)} cookies exported")
        print(f"OUTPUT: {output_file}")
        print("=" * 60)
    else:
        print("No cookies found.")


if __name__ == "__main__":
    main()
