import { Component, OnInit } from '@angular/core';
import { StatsService } from '../service/stats.service';
import { BehaviorSubject, Observable, map, switchMap, tap } from 'rxjs';

interface Stats {
  id: string;
  channel: string;
  payload: {
    name: string;
    cores: string;
    tag: string;
    hashrate: string;
    good_hashrate: string;
  }
}

@Component({
  selector: 'app-home',
  templateUrl: 'home.page.html',
  styleUrls: ['home.page.scss'],
})
export class HomePage implements OnInit {

  private subject: BehaviorSubject<Stats> = new BehaviorSubject<Stats>({} as Stats);
  private payload = this.subject.asObservable();

  public stats: Stats = {
    id: '',
    channel: '',
    payload: {}
  } as Stats;

  constructor(private readonly statsService: StatsService) { }

  ngOnInit(): void {
    this.payload.subscribe(res => console.log(res))
    setInterval(() => {
      this.statsService
        .rpc<string>('get_stats', ["d1H1tqHSoRQFumLVxg28akPHHXcks6FWTZcpKDUB8iDfmNL8J"])
        .pipe(
          map(res => JSON.parse(res)),
          tap((res: Stats) => this.stats = res)
        ).subscribe();
    }, 5000);
  }
}
