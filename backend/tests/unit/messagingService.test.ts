import { MessagingService, MessageType } from '../../src/services/messagingService';
import { Pool, PoolClient } from 'pg';

jest.mock('pg');
jest.mock('../../src/services/playerBlockService', () => ({
  __esModule: true,
  default: {
    isBlockedEither: jest.fn().mockResolvedValue(false),
  },
}));

const mockPlayerBlockService = require('../../src/services/playerBlockService').default;

describe('MessagingService', () => {
  let messagingService: MessagingService;
  let mockPool: any;
  let mockClient: any;

  beforeEach(() => {
    mockClient = {
      query: jest.fn(),
      release: jest.fn(),
    };

    mockPool = {
      query: jest.fn(),
      connect: jest.fn().mockResolvedValue(mockClient),
    };

    messagingService = new MessagingService(mockPool);
  });

  afterEach(() => {
    jest.clearAllMocks();
  });

  describe('sendMessage', () => {
    it('should send a player message successfully', async () => {
      const mockMessage = {
        id: 1,
        from_user_id: 1,
        to_user_id: 2,
        subject: 'Test Subject',
        content: 'Test Content',
        message_type: 'player_message',
        is_read: false,
        created_at: new Date(),
        metadata: null,
      };

      mockClient.query
        .mockResolvedValueOnce({ rows: [] } as any) // BEGIN
        .mockResolvedValueOnce({ rows: [mockMessage] } as any) // INSERT
        .mockResolvedValueOnce({ rows: [] } as any); // COMMIT

      const result = await messagingService.sendMessage({
        fromUserId: 1,
        toUserId: 2,
        subject: 'Test Subject',
        content: 'Test Content',
        messageType: MessageType.PLAYER_MESSAGE,
      });

      expect(result).toHaveProperty('id', 1);
      expect(result).toHaveProperty('subject', 'Test Subject');
      expect(mockClient.query).toHaveBeenCalledWith('BEGIN');
      expect(mockClient.query).toHaveBeenCalledWith('COMMIT');
      expect(mockClient.release).toHaveBeenCalled();
    });

    it('should reject when players block each other', async () => {
      mockPlayerBlockService.isBlockedEither.mockResolvedValueOnce(true);

      await expect(
        messagingService.sendMessage({
          fromUserId: 1,
          toUserId: 2,
          subject: 'Hello',
          content: 'Test',
          messageType: MessageType.PLAYER_MESSAGE,
        })
      ).rejects.toThrow(/blocked/i);

      expect(mockClient.query).toHaveBeenCalledWith('ROLLBACK');
    });

    it('should send system notification without sender', async () => {
      const mockMessage = {
        id: 2,
        from_user_id: null,
        to_user_id: 3,
        subject: 'System Alert',
        content: 'Maintenance scheduled',
        message_type: 'system_notification',
        is_read: false,
        created_at: new Date(),
        metadata: null,
      };

      mockClient.query
        .mockResolvedValueOnce({ rows: [] } as any)
        .mockResolvedValueOnce({ rows: [mockMessage] } as any)
        .mockResolvedValueOnce({ rows: [] } as any);

      const result = await messagingService.sendMessage({
        toUserId: 3,
        subject: 'System Alert',
        content: 'Maintenance scheduled',
        messageType: MessageType.SYSTEM_NOTIFICATION,
      });

      expect(result.fromUserId).toBeNull();
      expect(result.messageType).toBe('system_notification');
    });

    it('should include metadata when provided', async () => {
      const metadata = { combatId: 123, winner: 'attacker' };
      const mockMessage = {
        id: 3,
        from_user_id: null,
        to_user_id: 1,
        subject: 'Combat Report',
        content: 'Battle results',
        message_type: 'combat_report',
        is_read: false,
        created_at: new Date(),
        metadata: JSON.stringify(metadata),
      };

      mockClient.query
        .mockResolvedValueOnce({ rows: [] } as any)
        .mockResolvedValueOnce({ rows: [mockMessage] } as any)
        .mockResolvedValueOnce({ rows: [] } as any);

      const result = await messagingService.sendMessage({
        toUserId: 1,
        subject: 'Combat Report',
        content: 'Battle results',
        messageType: MessageType.COMBAT_REPORT,
        metadata,
      });

      expect(result.metadata).toEqual(metadata);
    });

    it('should rollback on error', async () => {
      mockClient.query
        .mockResolvedValueOnce({ rows: [] } as any) // BEGIN
        .mockRejectedValueOnce(new Error('Database error')); // INSERT fails

      await expect(
        messagingService.sendMessage({
          fromUserId: 1,
          toUserId: 2,
          subject: 'Test',
          content: 'Test',
          messageType: MessageType.PLAYER_MESSAGE,
        })
      ).rejects.toThrow('Database error');

      expect(mockClient.query).toHaveBeenCalledWith('ROLLBACK');
      expect(mockClient.release).toHaveBeenCalled();
    });
  });

  describe('getInbox', () => {
    it('should retrieve inbox messages with pagination', async () => {
      const mockMessages = [
        {
          id: 1,
          from_user_id: 2,
          to_user_id: 1,
          from_username: 'sender',
          to_username: 'recipient',
          subject: 'Message 1',
          content: 'Content 1',
          message_type: 'player_message',
          is_read: false,
          created_at: new Date(),
          metadata: null,
        },
        {
          id: 2,
          from_user_id: 3,
          to_user_id: 1,
          from_username: 'sender2',
          to_username: 'recipient',
          subject: 'Message 2',
          content: 'Content 2',
          message_type: 'player_message',
          is_read: true,
          created_at: new Date(),
          metadata: null,
        },
      ];

      mockPool.query.mockResolvedValue({ rows: mockMessages } as any);

      const result = await messagingService.getInbox(1, 50, 0);

      expect(result).toHaveLength(2);
      expect(result[0]).toHaveProperty('subject', 'Message 1');
      expect(result[0]).toHaveProperty('isRead', false);
    });

    it('should filter by message type', async () => {
      mockPool.query.mockResolvedValue({ rows: [] } as any);

      await messagingService.getInbox(1, 50, 0, MessageType.COMBAT_REPORT);

      expect(mockPool.query).toHaveBeenCalledWith(
        expect.stringContaining('message_type = $2'),
        expect.arrayContaining([1, 'combat_report', 50, 0])
      );
    });

    it('should handle empty inbox', async () => {
      mockPool.query.mockResolvedValue({ rows: [] } as any);

      const result = await messagingService.getInbox(1, 50, 0);

      expect(result).toEqual([]);
    });
  });

  describe('getSentMessages', () => {
    it('should retrieve sent messages', async () => {
      const mockMessages = [
        {
          id: 5,
          from_user_id: 1,
          to_user_id: 2,
          from_username: 'sender',
          to_username: 'recipient',
          subject: 'Sent Message',
          content: 'Content',
          message_type: 'player_message',
          is_read: true,
          created_at: new Date(),
          metadata: null,
        },
      ];

      mockPool.query.mockResolvedValue({ rows: mockMessages } as any);

      const result = await messagingService.getSentMessages(1, 50, 0);

      expect(result).toHaveLength(1);
      expect(result[0]).toHaveProperty('fromUserId', 1);
    });
  });

  describe('getMessage', () => {
    it('should retrieve a specific message for recipient', async () => {
      const mockMessage = {
        id: 10,
        from_user_id: 2,
        to_user_id: 1,
        from_username: 'sender',
        to_username: 'recipient',
        subject: 'Test',
        content: 'Content',
        message_type: 'player_message',
        is_read: false,
        created_at: new Date(),
        metadata: null,
      };

      mockPool.query.mockResolvedValue({ rows: [mockMessage] } as any);

      const result = await messagingService.getMessage(10, 1);

      expect(result).not.toBeNull();
      expect(result?.id).toBe(10);
    });

    it('should return null if message not found', async () => {
      mockPool.query.mockResolvedValue({ rows: [] } as any);

      const result = await messagingService.getMessage(999, 1);

      expect(result).toBeNull();
    });

    it('should return null if user is not authorized', async () => {
      mockPool.query.mockResolvedValue({ rows: [] } as any);

      const result = await messagingService.getMessage(10, 999);

      expect(result).toBeNull();
    });
  });

  describe('markAsRead', () => {
    it('should mark message as read', async () => {
      mockPool.query.mockResolvedValue({ rows: [{ id: 10 }] } as any);

      const result = await messagingService.markAsRead(10, 1);

      expect(result).toBe(true);
      expect(mockPool.query).toHaveBeenCalledWith(
        expect.stringContaining('is_read = true'),
        [10, 1]
      );
    });

    it('should return false if already read', async () => {
      mockPool.query.mockResolvedValue({ rows: [] } as any);

      const result = await messagingService.markAsRead(10, 1);

      expect(result).toBe(false);
    });
  });

  describe('markAllAsRead', () => {
    it('should mark all messages as read', async () => {
      mockPool.query.mockResolvedValue({
        rows: [{ id: 1 }, { id: 2 }, { id: 3 }],
      } as any);

      const count = await messagingService.markAllAsRead(1);

      expect(count).toBe(3);
    });

    it('should filter by message type when provided', async () => {
      mockPool.query.mockResolvedValue({ rows: [{ id: 1 }] } as any);

      await messagingService.markAllAsRead(1, MessageType.COMBAT_REPORT);

      expect(mockPool.query).toHaveBeenCalledWith(
        expect.stringContaining('message_type = $2'),
        [1, 'combat_report']
      );
    });
  });

  describe('deleteMessage', () => {
    it('should delete message for authorized user', async () => {
      mockPool.query.mockResolvedValue({ rows: [{ id: 10 }] } as any);

      const result = await messagingService.deleteMessage(10, 1);

      expect(result).toBe(true);
    });

    it('should return false if message not found', async () => {
      mockPool.query.mockResolvedValue({ rows: [] } as any);

      const result = await messagingService.deleteMessage(999, 1);

      expect(result).toBe(false);
    });
  });

  describe('getUnreadCount', () => {
    it('should return unread message count', async () => {
      mockPool.query.mockResolvedValue({ rows: [{ count: '5' }] } as any);

      const count = await messagingService.getUnreadCount(1);

      expect(count).toBe(5);
    });

    it('should filter by message type', async () => {
      mockPool.query.mockResolvedValue({ rows: [{ count: '2' }] } as any);

      const count = await messagingService.getUnreadCount(1, MessageType.COMBAT_REPORT);

      expect(count).toBe(2);
      expect(mockPool.query).toHaveBeenCalledWith(
        expect.stringContaining('message_type = $2'),
        [1, 'combat_report']
      );
    });
  });

  describe('sendCombatReport', () => {
    it('should send combat reports to both parties', async () => {
      const sendMessageSpy = jest
        .spyOn(messagingService, 'sendMessage')
        .mockResolvedValue({} as any);

      const combatData = {
        winner: 'attacker',
        loot: { metal: 1000, crystal: 500, deuterium: 250 },
      };

      await messagingService.sendCombatReport(1, 2, combatData);

      expect(sendMessageSpy).toHaveBeenCalledTimes(2);
      expect(sendMessageSpy).toHaveBeenCalledWith(
        expect.objectContaining({
          toUserId: 1,
          messageType: MessageType.COMBAT_REPORT,
          metadata: expect.objectContaining({ perspective: 'attacker' }),
        })
      );
      expect(sendMessageSpy).toHaveBeenCalledWith(
        expect.objectContaining({
          toUserId: 2,
          messageType: MessageType.COMBAT_REPORT,
          metadata: expect.objectContaining({ perspective: 'defender' }),
        })
      );
    });
  });

  describe('sendEspionageReport', () => {
    it('should send espionage report to spy', async () => {
      const sendMessageSpy = jest
        .spyOn(messagingService, 'sendMessage')
        .mockResolvedValue({} as any);

      const espionageData = {
        resources: { metal: 10000, crystal: 5000 },
        detected: false,
      };

      await messagingService.sendEspionageReport(1, 2, espionageData);

      expect(sendMessageSpy).toHaveBeenCalledWith(
        expect.objectContaining({
          toUserId: 1,
          messageType: MessageType.ESPIONAGE_REPORT,
        })
      );
    });

    it('should notify defender if espionage detected', async () => {
      const sendMessageSpy = jest
        .spyOn(messagingService, 'sendMessage')
        .mockResolvedValue({} as any);

      const espionageData = {
        resources: { metal: 10000, crystal: 5000 },
        detected: true,
      };

      await messagingService.sendEspionageReport(1, 2, espionageData);

      expect(sendMessageSpy).toHaveBeenCalledTimes(2);
      expect(sendMessageSpy).toHaveBeenCalledWith(
        expect.objectContaining({
          toUserId: 2,
          subject: 'Espionage Detected',
        })
      );
    });
  });

  describe('sendSystemNotification', () => {
    it('should send system notification', async () => {
      const sendMessageSpy = jest
        .spyOn(messagingService, 'sendMessage')
        .mockResolvedValue({} as any);

      await messagingService.sendSystemNotification(
        1,
        'Maintenance',
        'Server will be down',
        { downtime: '2 hours' }
      );

      expect(sendMessageSpy).toHaveBeenCalledWith(
        expect.objectContaining({
          toUserId: 1,
          subject: 'Maintenance',
          content: 'Server will be down',
          messageType: MessageType.SYSTEM_NOTIFICATION,
          metadata: { downtime: '2 hours' },
        })
      );
    });
  });

  describe('sendAllianceCircular', () => {
    it('should send message to all alliance members except sender', async () => {
      mockPool.query.mockResolvedValue({
        rows: [{ id: 2 }, { id: 3 }, { id: 4 }],
      } as any);

      const sendMessageSpy = jest
        .spyOn(messagingService, 'sendMessage')
        .mockResolvedValue({} as any);

      const count = await messagingService.sendAllianceCircular(
        1,
        1,
        'Meeting',
        'Important meeting tonight'
      );

      expect(count).toBe(3);
      expect(sendMessageSpy).toHaveBeenCalledTimes(3);
      expect(mockPool.query).toHaveBeenCalledWith(
        expect.stringContaining('alliance_id = $1 AND id != $2'),
        [1, 1]
      );
    });
  });
});
