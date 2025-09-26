---
title: "TuiTalk"  
author: "Behr Tobias, Schönig Marc, Seidl Anian"  
theme:  
  override:  
    footer:  
      style: template  
      left: "Verteilte Systeme"  
      center: "TuiTalk"  
      right: "{current_slide} / {total_slides}"  
---

Gliederung
===
1. Zielsetzung
2. Architektur
3. Implementierung
4. Showcase
5. Weiterentwicklung
6. Reflexion
7. Fazit



Zielsetzung
===
- Erstellen einer Chat-Plattform
- Ereichbarkeit über das Terminal und den Browser
- Kommunikation über WebSockets
- Speichern der Nachrichten in einer Datenbank



Architektur
===

![image:width:50%](./media/architektur.png "architektur")



Implementierung - TalkProtokoll
===
```rust
pub struct TalkMessage {
    pub uuid: Uuid,
    pub username: String,
    pub text: String,
    pub room_id: i32,
    pub unixtime: u64
}
```
--> Probleme

- Wie signalisiert man eine Raumänderung?
- Wie werden Errors angezeigt?
- Wie kann man mehrere Nachrichten empfangen bei einem fetch?



Implementierung - TalkProtokoll
===
```rust
pub enum TalkProtocol {
    // Client -> Server Commands
    JoinRoom { room_id: i32, uuid: Uuid, username: String, unixtime: u64},
    LeaveRoom { room_id: i32, uuid: Uuid, username: String, unixtime: u64},
    ChangeName {uuid: Uuid, username: String, old_username: String, unixtime: u64},
    Fetch { room_id: i32, limit: i64, fetch_before: u64},
    LocalError { message: String },
    LocalInformation { message: String },

    // Server -> Client Events
    UserJoined { uuid: Uuid, username: String, room_id: i32, unixtime: u64 },
    UserLeft { uuid: Uuid, username: String, room_id: i32, unixtime: u64  },
    UsernameChanged {uuid: Uuid, username: String, old_username: String, unixtime: u64},
    History { text: Vec<TalkProtocol> },
    Error { code: String, message: String },

    // Server <-> Client
    PostMessage { message: TalkMessage },
}
```



Implementierung - Funktionen des Clients
===

Senden und Empfangen von Nachrichten
| Befehl | Beschreibung |
| --- | --- |
| **/help** | Zeigt Informationen zu den verfügbaren Befehlen |
| | |
| **/clear** | Löscht den Chatverlauf |
| | |
| **/name {String}** | Setzt den Nutzernamen |
| | |
| **/room {Integer}** | Wechselt den Raum |
| | |
| **/fetch {Integer}** | Holt, ausgehend von der ersten Nachricht im Chatverlauf, die vorherigen Nachrichten |


Implementierung - Backend
===

--> Probleme
- Synchronisation zwischen mehreren Backendinstanzen
- Raumwechsel durch Future Channel pro Thread / Client

Implementierung - Backend
===

--> Lösung
- Redis Publish / Subscribe
- Rust Channels (Future, Oneshot)

```rust
// room change with command: /room {Id}
async fn handle_join(
    room_id: &i32,
    room_tx: &UnboundedSender<(i32, oneshot::Sender<()>)>,
) -> Result<()> {
    let (ack_tx, ack_rx) = oneshot::channel(); // create temporary channel for acknowledgement
    room_tx.send((*room_id, ack_tx))?; // send room change
    ack_rx.await?; // wait for acknowledgement
    Ok(())
}
```

Showcase
===
**Wasm-Client**

- `http://aicon.dhbw-heidenheim.de:7777`
- Manche Browser verbieten ungeschützte WebSockets

**Tui-Client**

- Installation über cargo
    - `cargo install tuitalk`
    - hinzufügen zu path / in folder gehen
    - `tuitalk ws://aicon.dhbw-heidenheim.de:8079`

- Manuelle Installation
    - Repo clonen (`git clone https://github.com/itsAnian/TuiTalk.git`)
    - In den Rust Folder wechseln (`TuiTalk/rust`)
    - `cargo run -p client ws://aicon.dhbw-heidenheim.de:8079`


Weiterentwicklung
===
- Wasm löschen
- WebSocket austauschen durch WebSocket Secure
- Hinzufügen des Timestamp durch Server 
- Tui-Client erweitern
    - Namen der Teilnehmer pro Raum anzeigen
    - Scrollen bei der Nachrichteneingabe
    - Automatisches Nachrichtenfetching
    - /whisper Funktion
- (Peer-to-Peer?)



Reflexion
===
- Rust war inital sehr aufwendig zu lernen
- Wasm Client Entwicklung wurde begonnen
    - große Startschwierigkeiten
    - Negative Einstellung gegenüber Wasm -> Vernachlässigung
    - konnte nicht sein volles Potential eintfalten
- Redis kann mehr als Cache?!



Fazit
===
- Rust trotz hohem initalem Aufwand gut zu verwenden
- Funktionierende Chat-Plattform mit unterschiedlichen Clients implementiert
- Das Projekt wird auf für uns wichtiges reduziert und weiterentwickelt
