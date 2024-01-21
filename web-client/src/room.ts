interface WebSocketMessage {
    action: string;
    content?: string;
    from?: string;
    to?: string;
    users?: string[];
}

class RoomApp {
    private ws: WebSocket;
    private roomId: string;
    private userId: string;

    constructor() {
        const params = new URLSearchParams(window.location.search);
        this.roomId = params.get('roomId') || '';
        this.userId = params.get('userId') || '';
        this.ws = new WebSocket(`ws://localhost:8081/ws?roomId=${this.roomId}&userId=${this.userId}`);

        this.attachEventListeners();
        this.initializeWebSocket();
        this.displayRoomId();
    }

    private attachEventListeners(): void {
        document.getElementById('send-direct')?.addEventListener('click', () => this.sendDirectMessage());
        document.getElementById('send-broadcast')?.addEventListener('click', () => this.broadcastMessage());
    }

    private initializeWebSocket(): void {
        this.ws.onopen = () => {
            console.log("Connected to the WebSocket server.");
        };

        this.ws.onmessage = (event) => {
            const message: WebSocketMessage = JSON.parse(event.data);
            switch (message.action) {
                case 'updateUsers':
                    this.updateUsersList(message.users || []);
                    break;
                case 'directMessage':
                case 'broadcastMessage':
                    this.displayMessage(message);
                    break;
                case 'userList':
                    this.updateUsersList(message.users || []);
                    break;
                // Handle other actions/messages
            }
        };

        this.ws.onerror = (error) => {
            console.error("WebSocket error:", error);
        };
    }

    private sendDirectMessage(): void {
        const toUserId = (document.getElementById('direct-user-id') as HTMLInputElement).value;
        const content = (document.getElementById('message-content') as HTMLInputElement).value;
        if (!content.trim()) return;

        const message: WebSocketMessage = { action: 'sendDirect', to: toUserId, content };
        this.ws.send(JSON.stringify(message));
    }

    private broadcastMessage(): void {
        const content = (document.getElementById('message-content') as HTMLInputElement).value;
        if (!content.trim()) return;

        const message: WebSocketMessage = { action: 'sendBroadcast', content };
        this.ws.send(JSON.stringify(message));
    }

    // Inside the RoomApp class

    private updateUsersList(users: string[]): void {
        const usersList = document.getElementById('users-list');
        if (!usersList) return;

        usersList.innerHTML = ''; 
        users.forEach(user => {
            const userElement = document.createElement('li');
            userElement.textContent = user;
            userElement.className = 'user-item';
            usersList.appendChild(userElement);
        });
    }

    private displayRoomId(): void {
        const roomIdDisplay = document.getElementById('room-id-display');
        if (roomIdDisplay) {
            roomIdDisplay.textContent = this.roomId;
        }
    }
    private displayMessage(message: WebSocketMessage): void {
        // Implement functionality to display the message
        console.log(`Message from ${message.from}: ${message.content}`);
    }

    private requestUserList(): void {
        if (this.ws.readyState === WebSocket.OPEN) {
            this.ws.send(JSON.stringify({ action: "listUsers", room_id: this.roomId }));
        }
    }

}

new RoomApp();
