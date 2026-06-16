const WebSocket = require('ws');
const ws = new WebSocket('ws://localhost:8080/ws');

ws.on('open', function open() {
  console.log('Connected.');
  ws.send(JSON.stringify({
    type: 'Chat',
    payload: {
      message: 'ping'
    }
  }));
});

ws.on('message', function incoming(data) {
  console.log('Received:', data.toString());
});

ws.on('close', function close() {
  console.log('Disconnected.');
});
