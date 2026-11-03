/**
 * @generated SignedSource<<74c7e3daa8ac45ac5e5b1fc3511d7ff5>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type LocationEditQuery$variables = {
  id: string;
};
export type LocationEditQuery$data = {
  readonly location: {
    readonly badgeEmailsEnabled: boolean;
    readonly enabled: boolean;
    readonly gamificationEnabled: boolean;
    readonly id: string;
    readonly name: string;
    readonly nitcCompleteOnExport: boolean;
    readonly nitcEnabled: number | null | undefined;
  };
};
export type LocationEditQuery = {
  response: LocationEditQuery$data;
  variables: LocationEditQuery$variables;
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
    "concreteType": "Location",
    "kind": "LinkedField",
    "name": "location",
    "plural": false,
    "selections": [
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "id",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "name",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "enabled",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "nitcEnabled",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "nitcCompleteOnExport",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "gamificationEnabled",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "badgeEmailsEnabled",
        "storageKey": null
      }
    ],
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Fragment",
    "metadata": {
      "throwOnFieldError": true
    },
    "name": "LocationEditQuery",
    "selections": (v1/*:: as any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Operation",
    "name": "LocationEditQuery",
    "selections": (v1/*:: as any*/)
  },
  "params": {
    "cacheID": "0201edfe999969446395590cf3a371cc",
    "id": null,
    "metadata": {},
    "name": "LocationEditQuery",
    "operationKind": "query",
    "text": "query LocationEditQuery(\n  $id: ID!\n) {\n  location(id: $id) {\n    id\n    name\n    enabled\n    nitcEnabled\n    nitcCompleteOnExport\n    gamificationEnabled\n    badgeEmailsEnabled\n  }\n}\n"
  }
};
})();

(node as any).hash = "7d0e70162408ee4b7b2b7d5a7019517b";

export default node;
