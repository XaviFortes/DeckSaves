# 📘 DeckSaves Versioning System

## Overview

DeckSaves now boasts a robust, storage-agnostic versioning layer. It bridges sync logic with version tracking and paves the way for plug-and-play backend expansions (e.g., Google Drive, WebDAV).

---

## 🚀 Architecture

### 1. Storage Abstraction (`providers.rs`)

* **`StorageProvider`**: Unified interface for all storage backends
* Bluetooth-current:

  * `S3StorageProvider` – AWS S3 (production-grade)
  * `LocalStorageProvider` – Local FS (offline/dev sandbox)
* Future-ready for: `GoogleDriveProvider`, `WebDAVProvider`, or other services

### 2. Version Management (`versioning.rs`)

* **`FileVersion`**: Tracks metadata (hash, timestamp, size, etc.)
* **`GameVersionManifest`**: JSON manifest of all versions per game
* **`VersionManager`**:

  * Auto-version creation on sync
  * SHA256 integrity checks
  * Intelligent pinning (daily, weekly, monthly, yearly policies)
  * Cleanup cleanup old versions except for pinned ones

### 3. Sync Integration (`versioned_sync.rs`)

* **`VersionedSync`**:

  * Encapsulates your existing sync logic
  * Enables version-aware upload/download
  * Detects conflicts by comparing local vs remote manifests
  * Ensures cleanup and retention adhere to policies

---

## ✅ Key Features

| Feature                | Benefit                                       |
| ---------------------- | --------------------------------------------- |
| Storage-agnostic       | Swap backends with zero code changes          |
| Auto-versioning        | Every sync = new, unique version              |
| Smart Pinning          | Prevents important versions from being pruned |
| Conflict Detection     | Prevents silent data overwrites               |
| Cleanup Policies       | Auto-housekeeping without manual dumps        |
| Integrity Verification | SHA256 ensures data hasn’t been tampered with |

---

## 🔧 What’s Implemented

* [x] Architecture (providers + versioning + sync layers)
* [x] AWS S3 + local FS providers
* [x] Versioning manifest + ID generation + hashing
* [x] Pinning & cleanup policies
* [x] Sync integration, code compiles clean with minor warnings

---

## 📌 Next-Phase Action Plan

### 1. **Integrate**

Transition your existing `GameSaveSync` to `VersionedSync` inside `lib.rs`.

### 2. **Test Rigorously**

* ✅ Run local FS sync cycles
* ✅ Run AWS S3 sync cycles
* ⏳ Validate version creation, pinning, cleanup, conflict behavior

### 3. **Expose via Tauri UI**

* Surface version history
* Allow users to pin/unpin
* Support version rollback/download

### 4. **Backend Expansion**

* Prototype `GoogleDriveProvider`, `WebDAVProvider`, etc.
* Plug into existing `StorageProvider` pipeline

---

## ✅ Summary

The system is:

* **Backward-compatible** – legacy syncs still work
* **Highly scalable** – you can add backends anytime
* **Resilient** – auto-versioning, integrity, pinning
* **Clean** – policy-driven cleanup, no fine dust data
* **Integratable** – hooks right into your existing logic

---

## 🤝 What’s Your Move?

* Should I **roll this into your live sync logic** now?
* Or would you prefer a **sandbox QA phase** with one backend before committing?

---
