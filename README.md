# TuiTalk
## Setup
For the Setup you have to define a .env file based on the .env.example.
```env
POSTGRES_PASSWORD=password
POSTGRES_USER=username
POSTGRES_DB=database
POSTGRES_PORT=5432
POSTGRES_HOST=postgresdb
```
After that you can start the Tui client with entering the rust folder and execute
```bash 
cargo run -p client
```
If you want to connect to a specific websocket connection you can also define it as argument after the command. For example:
```bash 
cargo run -p client ws://localhost:8079
```

## Tui-Client
### Movement
To move you have the following commands:
- j/k for up/down
- J/K for 10 up / 10 down
- g/G to the end / top of the messages

### Sending messages
To send messages press i to enter the insert mode.
When you have finished your message you can press enter to send it.

### Commands
- `/help` shows all commands
- `/name {string}` sets the given string as Username
- `/room {int}` joins the room you definesed in int
- `/fetch {int}` fetches the given number of messages up from the first messages in your history
- `/clear` clears the local messages

## Wasm-Client
The Wasm-Client implements the same commands given in the Commands section for the Tui-Client, but the sending of messages is slightly diffrent and the movement is like you would expect it in a browser.
