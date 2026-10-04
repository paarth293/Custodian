import * as https from 'node:https';

export interface ApiRequest {
  endpoint: string;
  payload: unknown;
}

export interface ApiResponse {
  data: string;
}

/**
 * Handles communication with the remote ML API.
 * This is the ONLY component in Custodian allowed to touch the network.
 */
export class ApiBroker {
  private readonly apiUrl: string;
  private readonly apiKey: string;

  constructor(apiUrl: string, apiKey: string) {
    this.apiUrl = apiUrl;
    this.apiKey = apiKey;
  }

  /**
   * Forwards a request to the remote ML API via HTTPS.
   * Fails fast if the network is down so the Agent can instantly fall back to local mode.
   */
  async fetchRemoteIntelligence(req: ApiRequest): Promise<ApiResponse> {
    return new Promise((resolve, reject) => {
      const url = new URL(req.endpoint, this.apiUrl);
      const dataString = JSON.stringify(req.payload);

      const options: https.RequestOptions = {
        hostname: url.hostname,
        port: url.port || 443,
        path: url.pathname,
        method: 'POST',
        timeout: 3000, // Fail fast for offline fallback
        headers: {
          'Content-Type': 'application/json',
          'Content-Length': Buffer.byteLength(dataString),
          Authorization: `Bearer ${this.apiKey}`,
        },
      };

      const clientReq = https.request(options, (res) => {
        let responseData = '';
        res.on('data', (chunk) => {
          responseData += chunk;
        });
        res.on('end', () => {
          if (res.statusCode && res.statusCode >= 200 && res.statusCode < 300) {
            resolve({ data: responseData });
          } else {
            reject(new Error(`API Error: ${res.statusCode}`));
          }
        });
      });

      clientReq.on('error', (err) => {
        // ENOTFOUND, ETIMEDOUT, ECONNREFUSED usually indicate we are offline
        reject(new Error(`Network error (fallback to local): ${err.message}`));
      });

      clientReq.on('timeout', () => {
        clientReq.destroy();
        reject(new Error('Network timeout (fallback to local)'));
      });

      clientReq.write(dataString);
      clientReq.end();
    });
  }
}
