export {};

declare global {
  namespace NodeJS {
    interface ProcessEnv {
      POLYGONSCAN_API_KEY: string;
      MNEMONIC: string
    }
  }
}
