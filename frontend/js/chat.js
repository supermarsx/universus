/**
 * UNIVERSUS CHAT - Real-time Chat System
 * Handles chat channels, private messages, and real-time updates
 */

class UniversusChat {
  constructor() {
    this.socket = window.realtimeSocket || null;
    this.currentChannelId = null;
    this.currentConversationId = null;
    this.channels = [];
    this.conversations = [];
    this.onlinePlayers = [];
    this.currentUserId = null;
    this.currentUsername = null;
    
    this.init();
  }

  async init() {
    // Get current user info
    const userInfo = await this.getCurrentUser();
    if (!userInfo) {
      window.location.href = '/';
      return;
    }
    
    this.currentUserId = userInfo.id;
    this.currentUsername = userInfo.username;
    
    // Load channels and conversations
    await this.loadChannels();
    await this.loadConversations();
    await this.loadOnlinePlayers();
    
    // Setup Socket.io listeners
    this.setupSocketListeners();
    
    // Setup UI event listeners
    this.setupUIListeners();
    
    // Auto-select first channel
    if (this.channels.length > 0) {
      this.selectChannel(this.channels[0].id);
    }
  }

  async getCurrentUser() {
    try {
      const response = await fetch('/api/users/me', {
        headers: { 'Authorization': `Bearer ${localStorage.getItem('jwt_token')}` }
      });
      if (!response.ok) return null;
      return await response.json();
    } catch (error) {
      console.error('Failed to get user info:', error);
      return null;
    }
  }

  async loadChannels() {
    try {
      const response = await fetch('/api/realtime/chat/channels', {
        headers: { 'Authorization': `Bearer ${localStorage.getItem('jwt_token')}` }
      });
      const data = await response.json();
      this.channels = data.channels || [];
      this.renderChannels();
    } catch (error) {
      console.error('Failed to load channels:', error);
    }
  }

  async loadConversations() {
    try {
      const response = await fetch('/api/realtime/chat/conversations?limit=20', {
        headers: { 'Authorization': `Bearer ${localStorage.getItem('jwt_token')}` }
      });
      const data = await response.json();
      this.conversations = data.conversations || [];
      this.renderConversations();
    } catch (error) {
      console.error('Failed to load conversations:', error);
    }
  }

  async loadOnlinePlayers() {
    try {
      const response = await fetch('/api/realtime/players/online?limit=50', {
        headers: { 'Authorization': `Bearer ${localStorage.getItem('jwt_token')}` }
      });
      const data = await response.json();
      this.onlinePlayers = data.players || [];
      this.renderOnlinePlayers();
    } catch (error) {
      console.error('Failed to load online players:', error);
    }
  }

  renderChannels() {
    const list = document.getElementById('channel-list');
    if (!list) return;
    
    list.innerHTML = this.channels.map(channel => `
      <div class="channel-item" data-channel-id="${channel.id}" onclick="chat.selectChannel(${channel.id})">
        <div class="channel-name">${channel.channel_name}</div>
        <div class="channel-type" style="font-size: 11px; color: #999;">${channel.channel_type}</div>
      </div>
    `).join('');
  }

  renderConversations() {
    const list = document.getElementById('conversation-list');
    if (!list) return;
    
    if (this.conversations.length === 0) {
      list.innerHTML = '<p style="font-size: 11px; color: #999; padding: 10px;">No conversations yet</p>';
      return;
    }
    
    list.innerHTML = this.conversations.map(conv => `
      <div class="conversation-item" data-conversation-id="${conv.id}" onclick="chat.selectConversation(${conv.id})">
        <div class="username">${conv.other_username}</div>
        ${conv.last_message ? `<div class="last-message">${conv.last_message}</div>` : ''}
        ${conv.unread_count > 0 ? `<span class="unread-badge">${conv.unread_count}</span>` : ''}
      </div>
    `).join('');
  }

  renderOnlinePlayers() {
    const container = document.getElementById('online-players');
    if (!container) return;
    
    const count = this.onlinePlayers.length;
    container.innerHTML = `
      <div class="online-count">${count} player${count !== 1 ? 's' : ''} online</div>
      ${this.onlinePlayers.slice(0, 20).map(player => `
        <div class="player-item" onclick="chat.startPrivateMessage('${player.username}')">
          <span class="player-status status-${player.status}"></span>
          ${player.username}
          ${player.alliance_tag ? `<span style="color: var(--secondary-color); font-size: 10px;">[${player.alliance_tag}]</span>` : ''}
        </div>
      `).join('')}
    `;
  }

  async selectChannel(channelId) {
    this.currentChannelId = channelId;
    this.currentConversationId = null;
    
    const channel = this.channels.find(c => c.id === channelId);
    if (!channel) return;
    
    // Update UI
    document.getElementById('chat-title').textContent = channel.channel_name;
    document.getElementById('chat-input').disabled = false;
    document.getElementById('send-btn').disabled = false;
    
    // Mark active channel
    document.querySelectorAll('.channel-item').forEach(el => {
      el.classList.toggle('active', parseInt(el.dataset.channelId) === channelId);
    });
    document.querySelectorAll('.conversation-item').forEach(el => {
      el.classList.remove('active');
    });
    
    // Subscribe to channel via Socket.io
    if (this.socket) {
      this.socket.emit('chat:subscribe', channelId);
    }
    
    // Load chat history
    await this.loadChatHistory(channelId);
    
    // Update channel info
    document.getElementById('channel-info').innerHTML = `
      <p><strong>Channel:</strong> ${channel.channel_name}</p>
      <p><strong>Type:</strong> ${channel.channel_type}</p>
      <p><strong>Description:</strong> ${channel.description || 'N/A'}</p>
      <p style="font-size: 11px; color: #999;">Rate limit: ${channel.rate_limit_seconds}s between messages</p>
    `;
  }

  async selectConversation(conversationId) {
    this.currentConversationId = conversationId;
    this.currentChannelId = null;
    
    const conversation = this.conversations.find(c => c.id === conversationId);
    if (!conversation) return;
    
    // Update UI
    document.getElementById('chat-title').textContent = `PM with ${conversation.other_username}`;
    document.getElementById('chat-input').disabled = false;
    document.getElementById('send-btn').disabled = false;
    
    // Mark active conversation
    document.querySelectorAll('.conversation-item').forEach(el => {
      el.classList.toggle('active', parseInt(el.dataset.conversationId) === conversationId);
    });
    document.querySelectorAll('.channel-item').forEach(el => {
      el.classList.remove('active');
    });
    
    // Subscribe to conversation
    if (this.socket) {
      this.socket.emit('pm:subscribe', conversationId);
    }
    
    // Load messages
    await this.loadPrivateMessages(conversationId);
    
    // Mark as read
    await this.markConversationAsRead(conversationId);
  }

  async loadChatHistory(channelId) {
    try {
      const response = await fetch(`/api/realtime/chat/channels/${channelId}/messages?limit=50`, {
        headers: { 'Authorization': `Bearer ${localStorage.getItem('jwt_token')}` }
      });
      const data = await response.json();
      
      this.renderMessages(data.messages || []);
    } catch (error) {
      console.error('Failed to load chat history:', error);
    }
  }

  async loadPrivateMessages(conversationId) {
    try {
      const response = await fetch(`/api/realtime/chat/conversations/${conversationId}/messages?limit=50`, {
        headers: { 'Authorization': `Bearer ${localStorage.getItem('jwt_token')}` }
      });
      const data = await response.json();
      
      this.renderMessages(data.messages || []);
    } catch (error) {
      console.error('Failed to load private messages:', error);
    }
  }

  renderMessages(messages) {
    const container = document.getElementById('chat-messages');
    if (!container) return;
    
    if (messages.length === 0) {
      container.innerHTML = '<div class="chat-welcome"><p>No messages yet. Start the conversation!</p></div>';
      return;
    }
    
    container.innerHTML = messages.map(msg => this.createMessageHTML(msg)).join('');
    container.scrollTop = container.scrollHeight;
  }

  createMessageHTML(msg) {
    const isOwnMessage = msg.user_id === this.currentUserId || msg.sender_id === this.currentUserId;
    const username = msg.username || msg.sender_username || 'Unknown';
    const messageClass = isOwnMessage ? 'chat-message own-message' : 'chat-message';
    
    return `
      <div class="${messageClass}" data-message-id="${msg.id}">
        <div class="message-header">
          <span class="username">${username}</span>
          ${msg.alliance_tag ? `<span class="alliance-tag">[${msg.alliance_tag}]</span>` : ''}
          <span class="timestamp">${this.formatTime(msg.created_at)}</span>
        </div>
        <div class="message-content">${this.escapeHTML(msg.message)}</div>
      </div>
    `;
  }

  appendMessage(msg) {
    const container = document.getElementById('chat-messages');
    if (!container) return;
    
    // Remove welcome message if exists
    const welcome = container.querySelector('.chat-welcome');
    if (welcome) welcome.remove();
    
    // Append new message
    container.insertAdjacentHTML('beforeend', this.createMessageHTML(msg));
    container.scrollTop = container.scrollHeight;
  }

  async sendMessage() {
    const input = document.getElementById('chat-input');
    const message = input.value.trim();
    
    if (!message) return;
    
    if (this.currentChannelId) {
      // Send to channel via Socket.io
      if (this.socket) {
        this.socket.emit('chat:message', {
          channelId: this.currentChannelId,
          message: message
        });
      }
    } else if (this.currentConversationId) {
      // Send private message
      const conversation = this.conversations.find(c => c.id === this.currentConversationId);
      if (conversation && this.socket) {
        this.socket.emit('pm:send', {
          receiverId: conversation.other_user_id,
          message: message
        });
      }
    }
    
    input.value = '';
  }

  async markConversationAsRead(conversationId) {
    if (!this.socket) return;
    this.socket.emit('pm:mark_read', conversationId);
    
    // Update local conversation
    const conv = this.conversations.find(c => c.id === conversationId);
    if (conv) {
      conv.unread_count = 0;
      this.renderConversations();
    }
  }

  startPrivateMessage(username) {
    // Find or create conversation
    // For now, just show a simple prompt
    const message = prompt(`Send message to ${username}:`);
    if (message && message.trim()) {
      // TODO: Implement creating new conversation
      alert('Private messaging feature coming soon!');
    }
  }

  setupSocketListeners() {
    if (!this.socket) {
      console.warn('Socket not initialized');
      return;
    }
    
    // Chat messages
    this.socket.on('chat:new_message', (data) => {
      if (data.channelId === this.currentChannelId) {
        this.appendMessage({
          id: data.messageId,
          user_id: data.userId,
          username: data.username,
          alliance_tag: data.allianceTag,
          message: data.message,
          created_at: data.timestamp
        });
      }
    });
    
    // Private messages
    this.socket.on('pm:new_message', (data) => {
      if (data.conversationId === this.currentConversationId) {
        this.appendMessage({
          id: data.messageId,
          sender_id: data.senderId,
          sender_username: data.senderUsername,
          message: data.message,
          created_at: data.timestamp
        });
      } else {
        // Show notification
        this.showNotification('New Private Message', `From ${data.senderUsername}`);
      }
      
      // Refresh conversations list
      this.loadConversations();
    });
    
    // Player status changes
    this.socket.on('player:status_change', (data) => {
      this.loadOnlinePlayers();
    });
    
    // Typing indicator
    this.socket.on('pm:user_typing', (data) => {
      if (data.conversationId === this.currentConversationId) {
        this.showTypingIndicator(data.username);
      }
    });
  }

  setupUIListeners() {
    // Send button
    const sendBtn = document.getElementById('send-btn');
    if (sendBtn) {
      sendBtn.addEventListener('click', () => this.sendMessage());
    }
    
    // Chat input - Enter key
    const input = document.getElementById('chat-input');
    if (input) {
      input.addEventListener('keypress', (e) => {
        if (e.key === 'Enter' && !e.shiftKey) {
          e.preventDefault();
          this.sendMessage();
        }
      });
      
      // Typing indicator for PMs
      let typingTimeout;
      input.addEventListener('input', () => {
        if (this.currentConversationId && this.socket) {
          clearTimeout(typingTimeout);
          
          const conversation = this.conversations.find(c => c.id === this.currentConversationId);
          if (conversation) {
            this.socket.emit('pm:typing', {
              conversationId: this.currentConversationId,
              receiverId: conversation.other_user_id
            });
          }
          
          typingTimeout = setTimeout(() => {
            // Stop typing indicator after 3 seconds
          }, 3000);
        }
      });
    }
  }

  showTypingIndicator(username) {
    const container = document.getElementById('chat-messages');
    if (!container) return;
    
    // Remove existing indicator
    const existing = container.querySelector('.typing-indicator');
    if (existing) existing.remove();
    
    // Add new indicator
    const indicator = document.createElement('div');
    indicator.className = 'typing-indicator';
    indicator.textContent = `${username} is typing...`;
    container.appendChild(indicator);
    
    // Remove after 3 seconds
    setTimeout(() => indicator.remove(), 3000);
  }

  showNotification(title, message) {
    if ('Notification' in window && Notification.permission === 'granted') {
      new Notification(title, { body: message });
    }
  }

  formatTime(timestamp) {
    const date = new Date(timestamp);
    const now = new Date();
    const diffMs = now - date;
    const diffMins = Math.floor(diffMs / 60000);
    
    if (diffMins < 1) return 'Just now';
    if (diffMins < 60) return `${diffMins}m ago`;
    if (diffMins < 1440) return `${Math.floor(diffMins / 60)}h ago`;
    return date.toLocaleDateString();
  }

  escapeHTML(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
  }
}

// Initialize chat when page loads
let chat;
document.addEventListener('DOMContentLoaded', () => {
  chat = new UniversusChat();
});

// Helper functions for onclick handlers
function openTradePanel() {
  window.location.href = '/trade';
}

function openFleetPanel() {
  window.location.href = '/fleet';
}

function openNotifications() {
  window.location.href = '/notifications';
}

function closePMModal() {
  document.getElementById('pm-modal').style.display = 'none';
}

function sendPrivateMessage() {
  // TODO: Implement PM sending
  alert('Private message sending not yet implemented');
  closePMModal();
}
