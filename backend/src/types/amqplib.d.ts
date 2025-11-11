declare module 'amqplib' {
  export type ConsumeMessage = {
    content: Buffer;
  } | null;

  export type Channel = {
    assertQueue(queue: string, opts?: any): Promise<any>;
    sendToQueue(queue: string, content: Buffer, opts?: any): boolean;
    consume(queue: string, onMessage: (msg: ConsumeMessage) => void, opts?: any): Promise<any>;
    ack(msg: ConsumeMessage): void;
    nack(msg: ConsumeMessage, allUpTo?: boolean, requeue?: boolean): void;
    create: any;
  };

  export type Connection = {
    createChannel(): Promise<Channel>;
    on(event: string, handler: (...args: any[]) => void): void;
    close(): Promise<void>;
  };

  const amqp: {
    connect(url: string): Promise<Connection>;
  };

  export default amqp;
}
