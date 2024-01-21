// import { WebSocketService } from './api/websocketService';

class MainApp {
    // private webSocketService: WebSocketService;

    constructor() {
        // this.webSocketService = new WebSocketService("ws://");
        this.initializeUI();
    }

    private initializeUI(): void {
        const joinRoomButton = document.getElementById("join-room");
        if (joinRoomButton === null) {
            alert('oops');
        } else {
            joinRoomButton.addEventListener("click", () => this.joinRoom());
        }
    }

    private joinRoom(): void {
        const roomId = (document.getElementById("room-id") as HTMLInputElement).value;
        const userId = (document.getElementById("user-id") as HTMLInputElement).value;
        if (!roomId || !userId) {
            alert('Room ID or User ID is empty. Please enter both values.');
            return;
        }
        
        console.log(`Joined room: ${roomId}`);
        const ws = new WebSocket("ws://localhost:8081/ws"); 
        ws.onopen = () => {
            console.log("Connected to the server");
            ws.send(JSON.stringify({ action: "join", userId, roomId }));
            window.location.href = `/room.html?roomId=${roomId}&userId=${userId}`;
        };
        ws.onmessage = (event) => {
            let rDisplay = document.getElementById("room-display")
            if (rDisplay === null) {
                alert('oops');
            } else {
                console.log("Message from server:", event.data);
                rDisplay.innerText = event.data;
            }
        };


    }

    // Additional methods to handle incoming msgs
    // and update the UI...
}

document.addEventListener("DOMContentLoaded", () => {
    new MainApp();
});

