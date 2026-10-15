/**
 * @generated SignedSource<<7477ad0a581697be9db2d038019a419e>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type ActivityListTableRemindMutation$variables = {
  id: string;
};
export type ActivityListTableRemindMutation$data = {
  readonly sendPeriodEditLink: string;
};
export type ActivityListTableRemindMutation = {
  response: ActivityListTableRemindMutation$data;
  variables: ActivityListTableRemindMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "id"
  }
],
v1 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "id",
        "variableName": "id"
      }
    ],
    "kind": "ScalarField",
    "name": "sendPeriodEditLink",
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Fragment",
    "metadata": null,
    "name": "ActivityListTableRemindMutation",
    "selections": (v1/*:: as any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Operation",
    "name": "ActivityListTableRemindMutation",
    "selections": (v1/*:: as any*/)
  },
  "params": {
    "cacheID": "73559204c83e77de4e36dd4433721263",
    "id": null,
    "metadata": {},
    "name": "ActivityListTableRemindMutation",
    "operationKind": "mutation",
    "text": "mutation ActivityListTableRemindMutation(\n  $id: ID!\n) {\n  sendPeriodEditLink(id: $id)\n}\n"
  }
};
})();

(node as any).hash = "4e58138dbf05f4be7809a6de05882782";

export default node;
