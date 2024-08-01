import { Component, Input, OnInit } from '@angular/core';
import { FormGroup } from '@angular/forms';

import { BaseComponent } from '@shared/bases/base.component';
import { WebFormService } from '@shared/services/web-form.service';

@Component({
  selector: 'web-client-pre-connection-blob-control',
  templateUrl: 'pre-connection-blob-control.component.html',
  styleUrls: ['pre-connection-blob-control.component.scss'],
})
export class PreConnectionBlobControlComponent extends BaseComponent implements OnInit {
  @Input() parentForm: FormGroup;
  @Input() inputFormData: any;

  constructor(private formService: WebFormService) {
    super();
  }

  ngOnInit(): void {
    this.formService.addControlToForm(this.parentForm, 'preConnectionBlob', this.inputFormData, false);
  }
}
