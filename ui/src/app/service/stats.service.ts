import { HttpClient } from '@angular/common/http';
import { Injectable } from '@angular/core';
import { Observable, map } from 'rxjs';
import { environment } from 'src/environments/environment';

@Injectable({
  providedIn: 'root'
})
export class StatsService {

  private rpcId: number = 0;
  private url: string = environment.rpcConfig.url;

  constructor(private readonly http: HttpClient) {
  }

  /** JSON RPC Request */
  private req(method: string, params?: any[]) {
    return {
      jsonrpc: '2.0',
      id: this.rpcId,
      method: method,
      params: params || []
    };
  }

  /** Send a request to the node */
  public rpc<T>(method: string, params?: any[]): Observable<T> {
    const payload = this.req(method, params);
    this.rpcId++;
    return this.http.post(this.url, payload).pipe(
      map((res: any) => {
        if (res.error) throw res.error;
        return res.result;
      })
    );
  }

  getGlobalStats() {

    // setInterval(async () => {

    // }, 1000);
    // subject.next({ "jsonrpc": "2.0", "id": 0, "method": "get_stats", "params": ["d1H1tqHSoRQFumLVxg28akPHHXcks6FWTZcpKDUB8iDfmNL8J"] });
  }

}
