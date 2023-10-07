import { Injectable } from '@angular/core';
import { WebSocketSubject, webSocket } from 'rxjs/webSocket';

import { Observable } from 'rxjs';
import { filter, first, tap, switchMap, map } from 'rxjs/operators';

@Injectable({ providedIn: 'root' })
export class WebsocketProvider {
  // @ts-ignore
  private socket$: WebSocketSubject<any>;
  public observables: Observable<any>[] = [];

  constructor() { }

  /**
   * Check if a message is the subscription we want
   * @param msg The message returned by the node
   * @param subscription The subscription to map
   */
  private isSubscription(msg: any, subscription: string): msg is any {
    return !!msg.method
      && msg.method === 'override'
      && msg.params.subscription === subscription;
  }

  /** Return the response of an RPC Request */
  private response<T>(id: number) {
    return this.socket$.pipe(
      // filter((msg: any) => msg.method === 'override'),
      // first()
      // tap((msg: any) => console.log(msg))
    );
  }

  /**
   * Subscribe to the node for a specific subscription name
   * @param subscription The subscription name we want to subscribe to
   */
  private subscription<T>(subscription: string): Observable<any> {
    return this.socket$.pipe(
      filter(msg => this.isSubscription(msg, subscription))
    )
  }

  /**
   * Create a socket between the client and the node
   * @param url The url of the node to connect to
   */
  public create(url: string) {
    // this.socket$ = new WebSocketSubject({
    //   url: url,
    // });
    this.socket$ = webSocket(url);
  }

  /**
   * Send an RPC request to the node
   * @param payload The RPC request
   */
  public post<T = any>(payload: any): Observable<any> {
    this.socket$.next(payload);
    return this.response<T>(payload.id).pipe(
      tap(res => console.log(res)),
    );
  }

  /**
   * Subscribe to a SUB/PUB
   * @param payload The RPC request
   */
  public subscribe(payload: any) {
    this.socket$.next(payload);
    return this.response<string>(payload.id).pipe(
      tap(res => { if (res.error) throw res.error; }),      
      filter(res => res instanceof Object),
      tap(res => console.log(res)),
      map(res => res.result),
      switchMap(result => {
        return this.observables[result] = this.subscription(result);
      })
    );
  }

  // TODO
  public unsubscribe() {

  }
}