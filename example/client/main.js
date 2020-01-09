const JANUS_API_ROOT = 'ws://0.0.0.0:8188';

class ExamplePluginClient {
  constructor() {
    this.socket = null;
    this.transactions = {};
    this.transactionCounter = 0;
    this.sessionId = null;
    this.handleId = null;
  }

  async init() {
    this.socket = await this._connect();
    this.socket.addEventListener('message', evt => this._handleMessage(JSON.parse(evt.data)));

    let sessionResponse = await this._createSession();
    this.sessionId = sessionResponse.id;

    let handleResponse = await this._createHandle();
    this.handleId = handleResponse.id;
  }

  _connect() {
    return new Promise((resolve, _reject) => {
      let socket = new WebSocket(JANUS_API_ROOT, 'janus-protocol');
      socket.addEventListener('open', () => resolve(socket))
    });
  }

  _handleMessage(message) {
    switch (message.janus) {
      case 'ack':
        break;
      case 'success':
      case 'message':
        this._dispatchTransactionCallback(message.transaction, 'success', message.data);
        break;
      case 'error':
        this._dispatchTransactionCallback(message.transaction, 'error', message.error);
        break;
      default:
        console.error('Unknown message from Janus', message);
    }      
  }

  _dispatchTransactionCallback(transaction, name, arg) {
    let callback = this.transactions[transaction][name];

    if (callback) {
      delete this.transactions[transaction].resolve;
      callback(arg);
    } else {
      console.error(`No ${name} callback set for transaction ${transaction}`);
    }
  }

  _sendRequest(request) {
    return new Promise((resolve, reject) => {
      let transaction = `txn${this.transactionCounter++}`;
      this.transactions[transaction] = { success: resolve, error: reject };
      this.socket.send(JSON.stringify({ ...request, transaction }));
    })
  }

  _createSession() {
    return this._sendRequest({ janus: 'create' });
  }

  _createHandle() {
    return this._sendRequest({
      janus: 'attach',
      session_id: this.sessionId,
      plugin: 'janus.plugin.app_example'
    });
  }

  _sendMessage(body) {
    return this._sendRequest({
      janus: 'message',
      session_id: this.sessionId,
      handle_id: this.handleId,
      body
    });
  }

  _callMethod(method, args) {
    return this._sendMessage({ ...args, method });
  }

  ping(data) {
    return this._callMethod('ping', { data });
  }
}

let client = new ExamplePluginClient();
let pingBtn = document.getElementById('pingBtn');

pingBtn.addEventListener('click', async function() {
  try {
    let response = await client.ping('ping');
    console.log(response);
  } catch (err) {
    console.error(err);
  }
});

async function init() {
  try {
    await client.init();
    pingBtn.disabled = false;
  } catch (err) {
    console.error(err);
  }
}

init();
