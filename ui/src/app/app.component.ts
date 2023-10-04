import { Component } from '@angular/core';

@Component({
  selector: 'app-root',
  templateUrl: './app.component.html',
  styleUrls: ['./app.component.scss']
})
export class AppComponent {
  title = 'ui';

  constructor() {

  }

  getStats() {
    
  }

  private getMessage(message: string) {
    console.log(message);
  }
}
