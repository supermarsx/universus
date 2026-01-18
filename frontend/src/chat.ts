// @ts-nocheck
/**
 * UNIVERSUS CHAT - Real-time Chat System
 * Handles chat channels, private messages, and real-time updates
 */

import i18n from './i18n';

const CHAT_REACTIONS = [
  { type: 'thumbs_up', emoji: '👍', label: i18n.t('chat.reaction.thumbs_up', { defaultValue: 'Thumbs up' }) },
  { type: 'thumbs_down', emoji: '👎', label: i18n.t('chat.reaction.thumbs_down', { defaultValue: 'Thumbs down' }) },
  { type: 'rofl', emoji: '🤣', label: i18n.t('chat.reaction.rofl', { defaultValue: 'ROFL' }) },
  { type: 'clap', emoji: '👏', label: i18n.t('chat.reaction.clap', { defaultValue: 'Clap' }) },
  { type: 'angry', emoji: '😡', label: i18n.t('chat.reaction.angry', { defaultValue: 'Angry' }) },
  { type: 'cry', emoji: '😭', label: i18n.t('chat.reaction.cry', { defaultValue: 'Crying' }) },
];

class UniversusChat {
  constructor() {
    this.socket = window.realtimeSocket || null;
    this.currentChannelId = null;
    this.currentConversationId = null;
    this.channels = [];
    this.channelMap = new Map();
    this.conversations = [];
    this.onlinePlayers = [];
    this.currentUserId = null;
    this.currentUsername = null;
    this.isAdmin = false;
    this.activeMessages = [];
    this.pinnedMessages = [];
    this.announcements = [];
    this.messageCache = new Map();
    this.mutedUsers = new Set();
    this.loadMutedUsers();
    
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
    this.isAdmin = Boolean(
      userInfo.is_admin ||
      userInfo.isAdmin ||
      userInfo.is_moderator ||
      userInfo.isModerator ||
      (userInfo.admin_level && ['moderator', 'game_admin', 'super_admin'].includes(userInfo.admin_level))
    );
    
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
      const data = await response.json();
      return data.user || data;
    } catch (error) {
      console.error('Failed to get user info:', error);
      return null;
    }
  }

  async loadChannels() {
    try {
      const response = await fetch('/api/realtime/chat/channels', {
        headers: this.getAuthHeaders()
      });
      const data = await response.json();
      this.channels = data.channels || [];
      this.channelMap = new Map(this.channels.map(channel => [channel.id, channel]));
      this.renderChannels();
    } catch (error) {
      console.error('Failed to load channels:', error);
    }
  }

  async loadConversations() {
    try {
      const response = await fetch('/api/realtime/chat/conversations?limit=20', {
        headers: this.getAuthHeaders()
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
        headers: this.getAuthHeaders()
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
      list.innerHTML = `<p style="font-size: 11px; color: #999; padding: 10px;">${i18n.t('chat.noConversations', { defaultValue: 'No conversations yet' })}</p>`;
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
      <div class="online-count">${i18n.t('chat.playersOnline', { defaultValue: `${count} player${count !== 1 ? 's' : ''} online` })}</div>
      ${this.onlinePlayers.slice(0, 20).map(player => `
        <div class="player-item" onclick="chat.startPrivateMessage(${player.user_id}, '${player.username}')">
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
    this.currentChannel = channel;
    this.updateAdminControls(channel);
    
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
      <p><strong>${i18n.t('chat.channelLabel', { defaultValue: 'Channel:' })}</strong> ${channel.channel_name}</p>
      <p><strong>${i18n.t('chat.typeLabel', { defaultValue: 'Type:' })}</strong> ${channel.channel_type}</p>
      <p><strong>${i18n.t('chat.descriptionLabel', { defaultValue: 'Description:' })}</strong> ${channel.description || i18n.t('chat.na', { defaultValue: 'N/A' })}</p>
      <p style="font-size: 11px; color: #999;">${i18n.t('chat.rateLimit', { defaultValue: `Rate limit: ${channel.rate_limit_seconds}s between messages` })}</p>
    `;
  }

  async selectConversation(conversationId) {
    this.currentConversationId = conversationId;
    this.currentChannelId = null;
    this.updateAdminControls(null);
    
    const conversation = this.conversations.find(c => c.id === conversationId);
    if (!conversation) return;
    
    // Update UI
    document.getElementById('chat-title').textContent = i18n.t('chat.pmWith', { defaultValue: `PM with ${conversation.other_username}` });
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
        headers: this.getAuthHeaders()
      });
      const data = await response.json();
      const messages = data.messages || [];
      this.activeMessages = messages;
      this.pinnedMessages = data.pinnedMessages || [];
      this.announcements = data.announcements || [];
      this.sortPinnedMessages();
      this.sortAnnouncements();
      this.renderAnnouncements();
      this.renderPinnedMessages();
      this.renderMessages(messages);
      [...this.pinnedMessages, ...this.announcements].forEach((msg) => {
        if (msg?.id) {
          this.messageCache.set(msg.id, msg);
        }
      });
    } catch (error) {
      console.error('Failed to load chat history:', error);
    }
  }

  async loadPrivateMessages(conversationId) {
    try {
      const response = await fetch(`/api/realtime/chat/conversations/${conversationId}/messages?limit=50`, {
        headers: this.getAuthHeaders()
      });
      const data = await response.json();
      
      this.renderMessages(data.messages || []);
    } catch (error) {
      console.error('Failed to load private messages:', error);
    }
  }

  getAuthHeaders(includeJson = false) {
    const headers: Record<string, string> = {
      'Authorization': `Bearer ${localStorage.getItem('jwt_token')}`
    };
    if (includeJson) {
      headers['Content-Type'] = 'application/json';
    }
    return headers;
  }

  loadMutedUsers() {
    try {
      const stored = JSON.parse(localStorage.getItem('chatMutedUsers') || '[]');
      this.mutedUsers = new Set((stored || []).map((name) => name.toLowerCase()));
    } catch {
      this.mutedUsers = new Set();
    }
  }

  saveMutedUsers() {
    localStorage.setItem('chatMutedUsers', JSON.stringify(Array.from(this.mutedUsers)));
  }

  isUserMuted(username?: string) {
    if (!username) return false;
    return this.mutedUsers.has(username.toLowerCase());
  }

  renderMessages(messages = this.activeMessages) {
    const container = document.getElementById('chat-messages');
    if (!container) return;
    
    if (!messages || messages.length === 0) {
      this.activeMessages = [];
      this.messageCache.clear();
      container.innerHTML = `<div class="chat-welcome"><p>${i18n.t('chat.noMessagesYet', { defaultValue: 'No messages yet. Start the conversation!' })}</p></div>`;
      return;
    }
    
    this.activeMessages = messages;
    this.messageCache.clear();
    messages.forEach((msg) => {
      if (msg?.id) {
        this.messageCache.set(msg.id, msg);
      }
    });

    const rendered = messages
      .filter((msg) => !this.isUserMuted(msg.username || msg.sender_username))
      .map((msg) => this.createMessageHTML(msg));

    if (rendered.length === 0) {
      container.innerHTML = `<div class="chat-welcome"><p>${i18n.t('chat.noMessagesToDisplay', { defaultValue: 'No messages to display.' })}</p></div>`;
      return;
    }

    container.innerHTML = rendered.join('');
    container.scrollTop = container.scrollHeight;
  }

  createMessageHTML(msg) {
    if (!msg) return '';
    const isOwnMessage = msg.user_id === this.currentUserId || msg.sender_id === this.currentUserId;
    const username = msg.username || msg.sender_username || msg.systemUsername || i18n.t('chat.unknown', { defaultValue: 'Unknown' });
    let messageClass = 'chat-message';

    if (msg.system) {
      messageClass = 'chat-message system-message';
    } else if (isOwnMessage) {
      messageClass = 'chat-message own-message';
    }

    if (msg.is_announcement) {
      messageClass += ' announcement-message';
    } else if (msg.is_pinned) {
      messageClass += ' pinned-message';
    }

    const badges = this.renderMessageBadges(msg);
    const adminControls = this.renderMessageAdminActions(msg);
    const reactionBar = this.renderReactionBar(msg);
    const allianceTag = msg.alliance_tag ? `<span class="alliance-tag">[${this.escapeHTML(msg.alliance_tag)}]</span>` : '';

    return `
      <div class="${messageClass}" data-message-id="${msg.id}">
        <div class="message-header">
          <div class="message-meta">
            <span class="username">${this.escapeHTML(username)}</span>
            ${allianceTag}
            <span class="timestamp">${this.formatTime(msg.created_at)}</span>
            ${badges}
          </div>
          ${adminControls}
        </div>
        <div class="message-content">${this.formatMessageContent(msg.message)}</div>
        ${reactionBar}
      </div>
    `;
  }

  appendMessage(msg) {
    if (!msg) return;
    const isChannelMessage = typeof msg.channel_id === 'number';
    const isConversationMessage = typeof msg.conversation_id === 'number';

    this.messageCache.set(msg.id, msg);

    if (isChannelMessage) {
      if (this.currentChannelId !== msg.channel_id) {
        return;
      }
    } else if (isConversationMessage) {
      if (this.currentConversationId !== msg.conversation_id) {
        return;
      }
    }

    this.syncMessageAcrossCollections(msg, { appendToActive: true });
    if (isChannelMessage) {
      if (msg.is_pinned) {
        this.upsertPinnedMessage(msg);
      }
      if (msg.is_announcement) {
        this.upsertAnnouncement(msg);
      }
    }

    const container = document.getElementById('chat-messages');
    if (!container) return;
    
    // Remove welcome message if exists
    const welcome = container.querySelector('.chat-welcome');
    if (welcome) welcome.remove();
    
    if (this.isUserMuted(msg.username)) {
      return;
    }

    // Append new message
    container.insertAdjacentHTML('beforeend', this.createMessageHTML(msg));
    container.scrollTop = container.scrollHeight;
  }

  async sendMessage() {
    const input = document.getElementById('chat-input');
    const message = input.value.trim();
    
    if (!message) return;

     if (message.startsWith('/')) {
       await this.handleCommand(message);
       input.value = '';
       return;
     }
    
    const adminFlags = this.getAdminMessageFlags();

    if (this.currentChannelId) {
      // Send to channel via Socket.io
      if (this.socket) {
        this.socket.emit('chat:message', {
          channelId: this.currentChannelId,
          message: message,
          ...adminFlags,
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
    this.resetAdminMessageFlags();
  }

  async handleCommand(rawCommand: string) {
    const parts = rawCommand.slice(1).trim().split(/\s+/).filter(Boolean);
    const command = (parts.shift() || '').toLowerCase();

    switch (command) {
      case 'block':
        if (!parts[0]) {
          this.displaySystemMessage(i18n.t('chat.usage.block', { defaultValue: 'Usage: /block <username> [scope]' }));
          return;
        }
        await this.blockUserCommand(parts[0], parts[1]);
        break;
      case 'unblock':
        if (!parts[0]) {
          this.displaySystemMessage(i18n.t('chat.usage.unblock', { defaultValue: 'Usage: /unblock <username>' }));
          return;
        }
        await this.unblockUserCommand(parts[0], parts[1]);
        break;
      case 'mute':
        if (!parts[0]) {
          this.displaySystemMessage(i18n.t('chat.usage.mute', { defaultValue: 'Usage: /mute <username>' }));
          return;
        }
        this.muteUser(parts[0]);
        break;
      case 'unmute':
        if (!parts[0]) {
          this.displaySystemMessage(i18n.t('chat.usage.unmute', { defaultValue: 'Usage: /unmute <username>' }));
          return;
        }
        this.unmuteUser(parts[0]);
        break;
      case 'muted':
        this.listMutedUsers();
        break;
      case 'commands':
      case 'help':
        this.displaySystemMessage(
          i18n.t('chat.commandsList', { defaultValue: 'Commands: /block, /unblock, /mute, /unmute, /muted, /help' })
        );
        break;
      default:
        this.displaySystemMessage(i18n.t('chat.unknownCommand', { defaultValue: `Unknown command: /${command}` }));
    }
  }

  async blockUserCommand(username, scope = 'all') {
    try {
      const response = await fetch('/api/player-blocks', {
        method: 'POST',
        headers: this.getAuthHeaders(true),
        body: JSON.stringify({ username, scope }),
      });
      const data = await response.json();
      if (!response.ok) {
        throw new Error(data?.error || i18n.t('chat.failedBlock', { defaultValue: 'Failed to block player' }));
      }
      this.displaySystemMessage(i18n.t('chat.blocked', { defaultValue: `Blocked ${username} (${scope})`, username, scope }));
    } catch (error: any) {
      this.displaySystemMessage(error?.message || i18n.t('chat.failedBlock', { defaultValue: 'Failed to block player' }));
    }
  }

  async unblockUserCommand(identifier, scope?: string) {
    try {
      const url = `/api/player-blocks/${encodeURIComponent(identifier)}${
        scope ? `?scope=${scope}` : ''
      }`;
      const response = await fetch(url, {
        method: 'DELETE',
        headers: this.getAuthHeaders(true),
      });
      const data = await response.json();
      if (!response.ok) {
        throw new Error(data?.error || i18n.t('chat.failedUnblock', { defaultValue: 'Failed to unblock player' }));
      }
      this.displaySystemMessage(i18n.t('chat.unblocked', { defaultValue: `Unblocked ${identifier}`, identifier }));
    } catch (error: any) {
      this.displaySystemMessage(error?.message || i18n.t('chat.failedUnblock', { defaultValue: 'Failed to unblock player' }));
    }
  }

  muteUser(username) {
    this.mutedUsers.add(username.toLowerCase());
    this.saveMutedUsers();
    this.displaySystemMessage(i18n.t('chat.mutedLocal', { defaultValue: `${username} muted locally.` }));
  }

  unmuteUser(username) {
    const removed = this.mutedUsers.delete(username.toLowerCase());
    this.saveMutedUsers();
    this.displaySystemMessage(
      removed ? i18n.t('chat.unmuted', { defaultValue: `${username} unmuted.` }) : i18n.t('chat.wasNotMuted', { defaultValue: `${username} was not muted.` })
    );
  }

  listMutedUsers() {
    if (this.mutedUsers.size === 0) {
      this.displaySystemMessage(i18n.t('chat.muteListEmpty', { defaultValue: 'Mute list is empty.' }));
      return;
    }
    this.displaySystemMessage(
      i18n.t('chat.mutedUsersList', { defaultValue: `Muted users: ${Array.from(this.mutedUsers).join(', ')}` })
    );
  }

  getAdminMessageFlags() {
    if (!this.isAdmin) {
      return {};
    }
    const pinToggle = document.getElementById('send-pin') as HTMLInputElement;
    const announcementToggle = document.getElementById('send-announcement') as HTMLInputElement;
    const expiryInput = document.getElementById('announcement-expiry') as HTMLInputElement;
    const isWorld = this.isWorldChannel(this.currentChannelId);

    const pinMessage = !!(pinToggle && pinToggle.checked);
    let isAnnouncement = !!(announcementToggle && announcementToggle.checked && isWorld);

    if (announcementToggle && announcementToggle.checked && !isWorld) {
      this.displaySystemMessage(i18n.t('chat.announcementsLimited', { defaultValue: 'Announcements are limited to the world chat.' }));
      announcementToggle.checked = false;
      isAnnouncement = false;
    }

    let announcementExpiresAt;
    if (isAnnouncement && expiryInput && expiryInput.value) {
      const date = new Date(expiryInput.value);
      if (!Number.isNaN(date.getTime())) {
        announcementExpiresAt = date.toISOString();
      }
    }

    return {
      pinMessage,
      isAnnouncement,
      announcementExpiresAt,
    };
  }

  resetAdminMessageFlags() {
    if (!this.isAdmin) return;
    const pinToggle = document.getElementById('send-pin') as HTMLInputElement;
    const announcementToggle = document.getElementById('send-announcement') as HTMLInputElement;
    const expiryInput = document.getElementById('announcement-expiry') as HTMLInputElement;
    if (pinToggle) pinToggle.checked = false;
    if (announcementToggle) announcementToggle.checked = false;
    if (expiryInput) expiryInput.value = '';
  }

  isWorldChannel(channelId) {
    const channel = this.channelMap.get(channelId);
    return Boolean(channel && channel.channel_type === 'global');
  }

  updateAdminControls(channel) {
    const controls = document.getElementById('chat-admin-controls');
    if (!controls) return;

    if (!this.isAdmin) {
      controls.style.display = 'none';
      return;
    }

    if (!channel && this.currentChannelId === null) {
      controls.style.display = 'none';
      return;
    }

    controls.style.display = 'flex';
    const announcementToggle = document.getElementById('send-announcement') as HTMLInputElement;
    const expiryInput = document.getElementById('announcement-expiry') as HTMLInputElement;
    const announcementWrapper = controls.querySelector('[data-role="announcement"]');
    const isWorld = channel ? channel.channel_type === 'global' : this.isWorldChannel(this.currentChannelId);

    if (announcementToggle) {
      announcementToggle.disabled = !isWorld;
      if (!isWorld) {
        announcementToggle.checked = false;
      }
    }
    if (expiryInput) {
      expiryInput.disabled = !isWorld;
      if (!isWorld) {
        expiryInput.value = '';
      }
    }
    if (announcementWrapper) {
      announcementWrapper.classList.toggle('disabled', !isWorld);
    }
  }

  syncMessageAcrossCollections(message, options: { appendToActive?: boolean } = {}) {
    const { appendToActive = false } = options;
    if (!message || !message.id) return;

    let replaced = false;
    this.activeMessages = this.activeMessages.map((existing) => {
      if (existing.id === message.id) {
        replaced = true;
        return message;
      }
      return existing;
    });

    if (!replaced && appendToActive) {
      this.activeMessages.push(message);
    }

    this.messageCache.set(message.id, message);
  }

  updateMessageCollection(list, message, shouldInclude) {
    const index = list.findIndex((item) => item.id === message.id);
    if (shouldInclude) {
      if (index === -1) {
        list.unshift(message);
      } else {
        list[index] = message;
      }
    } else if (index !== -1) {
      list.splice(index, 1);
    }
  }

  sortPinnedMessages() {
    this.pinnedMessages.sort((a, b) => {
      const aDate = new Date(a.pinned_at || a.created_at || Date.now());
      const bDate = new Date(b.pinned_at || b.created_at || Date.now());
      return bDate.getTime() - aDate.getTime();
    });
  }

  sortAnnouncements() {
    this.announcements.sort((a, b) => {
      const aDate = new Date(a.created_at || Date.now());
      const bDate = new Date(b.created_at || Date.now());
      return bDate.getTime() - aDate.getTime();
    });
  }

  renderPinnedMessages() {
    const container = document.getElementById('chat-pinned');
    if (!container) return;
    if (!this.pinnedMessages.length) {
      container.style.display = 'none';
      container.innerHTML = '';
      return;
    }

    container.style.display = 'block';
    container.innerHTML = this.pinnedMessages
      .map((msg) => this.createPinnedCardHTML(msg))
      .join('');
  }

  renderAnnouncements() {
    const container = document.getElementById('chat-announcements');
    if (!container) return;
    if (!this.announcements.length) {
      container.style.display = 'none';
      container.innerHTML = '';
      return;
    }

    container.style.display = 'block';
    container.innerHTML = this.announcements
      .map((msg) => this.createAnnouncementCardHTML(msg))
      .join('');
  }

  createPinnedCardHTML(msg) {
    return `
      <div class="pinned-card" data-scroll-to="${msg.id}">
        <div class="pinned-meta">
          <span class="username">${this.escapeHTML(msg.username || i18n.t('chat.unknown', { defaultValue: 'Unknown' }))}</span>
          <span class="timestamp">${this.formatTime(msg.pinned_at || msg.created_at)}</span>
        </div>
        <div class="pinned-body">${this.formatMessageContent(msg.message)}</div>
      </div>
    `;
  }

  createAnnouncementCardHTML(msg) {
    return `
      <div class="announcement-card" data-scroll-to="${msg.id}">
        <div class="announcement-meta">
          <span class="username">${this.escapeHTML(msg.username || i18n.t('chat.unknown', { defaultValue: 'Unknown' }))}</span>
          <span class="timestamp">${this.formatTime(msg.created_at)}</span>
        </div>
        <div class="announcement-body">${this.formatMessageContent(msg.message)}</div>
      </div>
    `;
  }

  upsertPinnedMessage(msg) {
    const shouldInclude = Boolean(msg.is_pinned);
    this.updateMessageCollection(this.pinnedMessages, msg, shouldInclude);
    this.sortPinnedMessages();
    this.renderPinnedMessages();
  }

  upsertAnnouncement(msg) {
    const expiresAt = msg.announcement_expires_at ? new Date(msg.announcement_expires_at).getTime() : null;
    const isExpired = expiresAt && expiresAt < Date.now();
    const shouldInclude = Boolean(msg.is_announcement) && !isExpired;
    this.updateMessageCollection(this.announcements, msg, shouldInclude);
    this.sortAnnouncements();
    this.renderAnnouncements();
  }

  scrollToMessage(messageId) {
    const container = document.getElementById('chat-messages');
    if (!container) return;
    const target = container.querySelector(`.chat-message[data-message-id="${messageId}"]`);
    if (target) {
      target.classList.add('message-highlight');
      target.scrollIntoView({ behavior: 'smooth', block: 'center' });
      setTimeout(() => target.classList.remove('message-highlight'), 1500);
    }
  }

  renderMessageBadges(msg) {
    const badges: string[] = [];
    if (msg.is_announcement) {
      badges.push(`<span class="message-badge announcement">${i18n.t('chat.announcementBadge', { defaultValue: 'Announcement' })}</span>`);
    }
    if (msg.is_pinned) {
      badges.push(`<span class="message-badge pinned">${i18n.t('chat.pinnedBadge', { defaultValue: 'Pinned' })}</span>`);
    }
    return badges.join('');
  }

  renderMessageAdminActions(msg) {
    if (!this.isAdmin || !msg.id || !msg.channel_id) return '';
    const actions: string[] = [];
    const isPinned = Boolean(msg.is_pinned);
    actions.push(
      `<button class="message-action-btn" data-message-action="pin" data-message-id="${msg.id}" data-pinned="${isPinned}">
        ${isPinned ? i18n.t('chat.unpin', { defaultValue: 'Unpin' }) : i18n.t('chat.pin', { defaultValue: 'Pin' })}
      </button>`
    );

    if (this.isWorldChannel(msg.channel_id)) {
      const isAnnouncement = Boolean(msg.is_announcement);
      actions.push(
        `<button class="message-action-btn" data-message-action="announcement" data-message-id="${msg.id}" data-announcement="${isAnnouncement}">
          ${isAnnouncement ? i18n.t('chat.unmark', { defaultValue: 'Unmark' }) : i18n.t('chat.announcementLabel', { defaultValue: 'Announcement' })}
        </button>`
      );
    }

    if (!actions.length) return '';
    return `<div class="message-actions">${actions.join('')}</div>`;
  }

  renderReactionBar(msg) {
    if (!msg || !msg.id || !msg.channel_id) return '';
    const reactions = msg.reactions || {};
    const viewerReactions = msg.viewerReactions || [];

    const buttons = CHAT_REACTIONS.map((reaction) => {
      const count = reactions[reaction.type] || 0;
      const isActive = viewerReactions.includes(reaction.type);
      return `
        <button
          class="reaction-btn ${isActive ? 'active' : ''}"
          data-reaction-btn
          data-reaction="${reaction.type}"
          data-message-id="${msg.id}"
          title="${reaction.label}"
        >
          <span class="reaction-emoji">${reaction.emoji}</span>
          <span class="reaction-count">${count}</span>
        </button>
      `;
    }).join('');

    return `<div class="message-reactions" data-message-id="${msg.id}">${buttons}</div>`;
  }

  formatMessageContent(text = '') {
    return this.escapeHTML(text).replace(/\n/g, '<br>');
  }

  async toggleReaction(messageId, reactionType) {
    if (!messageId || !reactionType) return;
    try {
      const response = await fetch(`/api/realtime/chat/messages/${messageId}/reactions`, {
        method: 'POST',
        headers: this.getAuthHeaders(true),
        body: JSON.stringify({ reactionType }),
      });
      if (!response.ok) {
        throw new Error(i18n.t('chat.failedToggleReaction', { defaultValue: 'Failed to toggle reaction' }));
      }
      const data = await response.json();
      const existing = this.messageCache.get(messageId) || {};
      existing.reactions = data.reactions || {};
      existing.viewerReactions = data.viewerReactions || [];
      this.messageCache.set(messageId, existing);
      this.syncMessageAcrossCollections(existing);
      this.refreshMessage(messageId);
    } catch (error) {
      console.error('Failed to toggle reaction:', error);
    }
  }

  refreshMessage(messageId) {
    const container = document.querySelector(`.chat-message[data-message-id="${messageId}"]`);
    const message = this.messageCache.get(messageId);
    if (!container || !message) return;
    const wrapper = document.createElement('div');
    wrapper.innerHTML = this.createMessageHTML(message).trim();
    const next = wrapper.firstElementChild;
    if (next) {
      container.replaceWith(next);
    }
  }

  async handleMessageAction(action, messageId, target) {
    if (!action || !messageId) return;
    if (action === 'pin') {
      const isPinned = target?.dataset?.pinned === 'true';
      await this.setPinnedState(messageId, !isPinned);
    } else if (action === 'announcement') {
      const isAnnouncement = target?.dataset?.announcement === 'true';
      let expiresAt;
      if (!isAnnouncement) {
        const input = prompt(i18n.t('chat.announcementPrompt', { defaultValue: 'Announcement duration in minutes (leave blank for no expiry):' }));
        if (input) {
          const minutes = parseInt(input, 10);
          if (!Number.isNaN(minutes) && minutes > 0) {
            const date = new Date(Date.now() + minutes * 60000);
            expiresAt = date.toISOString();
          }
        }
      }
      await this.setAnnouncementState(messageId, !isAnnouncement, expiresAt);
    }
  }

  async setPinnedState(messageId, shouldPin) {
    try {
      const response = await fetch(`/api/realtime/chat/messages/${messageId}/pin`, {
        method: 'POST',
        headers: this.getAuthHeaders(true),
        body: JSON.stringify({ pinned: shouldPin }),
      });
      if (!response.ok) {
        throw new Error(i18n.t('chat.failedUpdatePin', { defaultValue: 'Failed to update pin' }));
      }
      const data = await response.json();
      const updated = data.message;
      this.messageCache.set(updated.id, updated);
      this.syncMessageAcrossCollections(updated);
      this.upsertPinnedMessage(updated);
      this.refreshMessage(updated.id);
    } catch (error) {
      console.error('Failed to update pin:', error);
      this.displaySystemMessage(i18n.t('chat.unableUpdatePin', { defaultValue: 'Unable to update pin state.' }));
    }
  }

  async setAnnouncementState(messageId, enabled, expiresAt) {
    try {
      const response = await fetch(`/api/realtime/chat/messages/${messageId}/announcement`, {
        method: 'POST',
        headers: this.getAuthHeaders(true),
        body: JSON.stringify({
          isAnnouncement: enabled,
          expiresAt: expiresAt || null,
        }),
      });
      if (!response.ok) {
        throw new Error(i18n.t('chat.failedUpdateAnnouncement', { defaultValue: 'Failed to update announcement' }));
      }
      const data = await response.json();
      const updated = data.message;
      this.messageCache.set(updated.id, updated);
      this.syncMessageAcrossCollections(updated);
      this.upsertAnnouncement(updated);
      this.refreshMessage(updated.id);
    } catch (error) {
      console.error('Failed to update announcement:', error);
      this.displaySystemMessage(i18n.t('chat.unableUpdateAnnouncement', { defaultValue: 'Unable to update announcement state.' }));
    }
  }

  displaySystemMessage(message: string) {
    const systemMessage = {
      id: `sys-${Date.now()}`,
      system: true,
      systemUsername: i18n.t('chat.systemUsername', { defaultValue: 'System' }),
      message,
      created_at: new Date().toISOString(),
    };
    this.appendMessage(systemMessage);
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

  startPrivateMessage(userId, username) {
    if (!userId || userId === this.currentUserId) {
      return;
    }

    const existing = this.conversations.find(
      (conv) => conv.other_user_id === userId || conv.other_username === username
    );
    if (existing) {
      this.selectConversation(existing.id);
      return;
    }

    this.openPrivateMessageModal(userId, username);
  }

  openPrivateMessageModal(userId, username) {
    const modal = document.getElementById('pm-modal');
    if (!modal) return;
    const recipientInput = document.getElementById('pm-recipient');
    const messageInput = document.getElementById('pm-message');
    if (recipientInput) {
      recipientInput.value = username || '';
      if (userId) {
        recipientInput.dataset.recipientId = String(userId);
      } else {
        delete recipientInput.dataset.recipientId;
      }
    }
    if (messageInput) {
      messageInput.value = '';
      messageInput.focus();
    }
    modal.style.display = 'block';
  }

  resolveRecipientId(username) {
    const normalized = String(username || '').trim().toLowerCase();
    if (!normalized) return null;

    const onlineMatch = this.onlinePlayers.find(
      (player) => player.username?.toLowerCase() === normalized
    );
    if (onlineMatch?.user_id) {
      return onlineMatch.user_id;
    }

    const conversationMatch = this.conversations.find(
      (conv) => conv.other_username?.toLowerCase() === normalized
    );
    if (conversationMatch?.other_user_id) {
      return conversationMatch.other_user_id;
    }

    return null;
  }

  async sendPrivateMessageFromModal() {
    const recipientInput = document.getElementById('pm-recipient');
    const messageInput = document.getElementById('pm-message');
    if (!recipientInput || !messageInput) return;

    const message = messageInput.value.trim();
    if (!message) return;

    let recipientId = parseInt(recipientInput.dataset.recipientId || '0', 10);
    if (!recipientId) {
      recipientId = this.resolveRecipientId(recipientInput.value) || 0;
    }

    if (!recipientId) {
      alert(i18n.t('chat.userNotFound', { defaultValue: 'User not found. Try an online player or existing conversation.' }));
      return;
    }

    try {
      const response = await fetch('/api/realtime/chat/private', {
        method: 'POST',
        headers: this.getAuthHeaders(true),
        body: JSON.stringify({ receiverId: recipientId, message }),
      });
      const data = await response.json();
      if (!response.ok) {
        throw new Error(data?.error || i18n.t('chat.failedSendPm', { defaultValue: 'Failed to send private message' }));
      }

      const privateMessage = data.message;
      await this.loadConversations();
      if (privateMessage?.conversation_id) {
        await this.selectConversation(privateMessage.conversation_id);
      }
      closePMModal();
    } catch (error) {
      console.error('Failed to send private message:', error);
      alert(error?.message || i18n.t('chat.failedSendPm', { defaultValue: 'Failed to send private message' }));
    }
  }

  setupSocketListeners() {
    if (!this.socket) {
      console.warn('Socket not initialized');
      return;
    }
    
    // Chat messages
    this.socket.on('chat:new_message', (data) => {
      if (!data) return;
      const payload = data.message
        ? data
        : {
            channelId: data.channelId,
            message: {
              id: data.messageId,
              user_id: data.userId,
              username: data.username,
              alliance_tag: data.allianceTag,
              message: data.message,
              created_at: data.timestamp,
              channel_id: data.channelId,
              reactions: {},
              viewerReactions: [],
            },
          };

      this.messageCache.set(payload.message.id, payload.message);
      if (payload.channelId === this.currentChannelId) {
        this.appendMessage(payload.message);
      }
    });

    this.socket.on('chat:message_pinned', ({ channelId, message }) => {
      if (!message) return;
      this.messageCache.set(message.id, message);
      if (channelId === this.currentChannelId) {
        this.syncMessageAcrossCollections(message);
        this.upsertPinnedMessage(message);
        this.refreshMessage(message.id);
      }
    });

    this.socket.on('chat:announcement_changed', ({ channelId, message }) => {
      if (!message) return;
      this.messageCache.set(message.id, message);
      if (channelId === this.currentChannelId) {
        this.syncMessageAcrossCollections(message);
        this.upsertAnnouncement(message);
        this.refreshMessage(message.id);
      }
    });

    this.socket.on('chat:reaction_update', ({ channelId, messageId, reactions }) => {
      if (channelId !== this.currentChannelId) return;
      const message = this.messageCache.get(messageId);
      if (message) {
        message.reactions = reactions || {};
        this.messageCache.set(messageId, message);
        this.refreshMessage(messageId);
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
          created_at: data.timestamp,
          conversation_id: data.conversationId,
        });
      } else {
        // Show notification
        this.showNotification(i18n.t('chat.newPmTitle', { defaultValue: 'New Private Message' }), i18n.t('chat.newPmFrom', { defaultValue: `From ${data.senderUsername}` }));
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

    const messagesContainer = document.getElementById('chat-messages');
    if (messagesContainer) {
      messagesContainer.addEventListener('click', (event) => {
        const target = (event.target as HTMLElement) || null;
        if (!target) return;
        const reactionBtn = target.closest('[data-reaction-btn]');
        if (reactionBtn) {
          const messageId = parseInt(reactionBtn.getAttribute('data-message-id') || '0', 10);
          const reactionType = reactionBtn.getAttribute('data-reaction');
          this.toggleReaction(messageId, reactionType);
          return;
        }
        const actionBtn = target.closest('[data-message-action]');
        if (actionBtn) {
          const messageId = parseInt(actionBtn.getAttribute('data-message-id') || '0', 10);
          const action = actionBtn.getAttribute('data-message-action');
          this.handleMessageAction(action, messageId, actionBtn);
        }
      });
    }

    const pinnedContainer = document.getElementById('chat-pinned');
    if (pinnedContainer) {
      pinnedContainer.addEventListener('click', (event) => {
        const target = (event.target as HTMLElement) || null;
        if (!target) return;
        const card = target.closest('[data-scroll-to]');
        if (card) {
          const messageId = parseInt(card.getAttribute('data-scroll-to') || '0', 10);
          this.scrollToMessage(messageId);
        }
      });
    }

    const announcementContainer = document.getElementById('chat-announcements');
    if (announcementContainer) {
      announcementContainer.addEventListener('click', (event) => {
        const target = (event.target as HTMLElement) || null;
        if (!target) return;
        const card = target.closest('[data-scroll-to]');
        if (card) {
          const messageId = parseInt(card.getAttribute('data-scroll-to') || '0', 10);
          this.scrollToMessage(messageId);
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
    indicator.textContent = i18n.t('chat.typing', { defaultValue: `${username} is typing...` });
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
    
    if (diffMins < 1) return i18n.t('chat.justNow', { defaultValue: 'Just now' });
    if (diffMins < 60) return i18n.t('chat.minutesAgo', { defaultValue: `${diffMins}m ago` });
    if (diffMins < 1440) return i18n.t('chat.hoursAgo', { defaultValue: `${Math.floor(diffMins / 60)}h ago` });
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
  if (!chat) return;
  chat.sendPrivateMessageFromModal();
}
