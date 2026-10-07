/**
 * @generated SignedSource<<f9e9b2f896ba784a8bdef008aab89bb6>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type LocationEditMutation$variables = {
  enabled: boolean;
  id: string;
  name: string;
  nitcCompleteOnExport?: boolean | null | undefined;
  nitcEnabled?: number | null | undefined;
};
export type LocationEditMutation$data = {
  readonly updateLocation: {
    readonly enabled: boolean;
    readonly id: string;
    readonly name: string;
    readonly nitcCompleteOnExport: boolean;
    readonly nitcEnabled: number | null | undefined;
  };
};
export type LocationEditMutation = {
  response: LocationEditMutation$data;
  variables: LocationEditMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "enabled"
},
v1 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "id"
},
v2 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "name"
},
v3 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "nitcCompleteOnExport"
},
v4 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "nitcEnabled"
},
v5 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "enabled",
        "variableName": "enabled"
      },
      {
        "kind": "Variable",
        "name": "id",
        "variableName": "id"
      },
      {
        "kind": "Variable",
        "name": "name",
        "variableName": "name"
      },
      {
        "kind": "Variable",
        "name": "nitcCompleteOnExport",
        "variableName": "nitcCompleteOnExport"
      },
      {
        "kind": "Variable",
        "name": "nitcEnabled",
        "variableName": "nitcEnabled"
      }
    ],
    "concreteType": "Location",
    "kind": "LinkedField",
    "name": "updateLocation",
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
      }
    ],
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": [
      (v0/*:: as any*/),
      (v1/*:: as any*/),
      (v2/*:: as any*/),
      (v3/*:: as any*/),
      (v4/*:: as any*/)
    ],
    "kind": "Fragment",
    "metadata": null,
    "name": "LocationEditMutation",
    "selections": (v5/*:: as any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [
      (v1/*:: as any*/),
      (v2/*:: as any*/),
      (v0/*:: as any*/),
      (v4/*:: as any*/),
      (v3/*:: as any*/)
    ],
    "kind": "Operation",
    "name": "LocationEditMutation",
    "selections": (v5/*:: as any*/)
  },
  "params": {
    "cacheID": "ae452a62945900435b3905298154fb6f",
    "id": null,
    "metadata": {},
    "name": "LocationEditMutation",
    "operationKind": "mutation",
    "text": "mutation LocationEditMutation(\n  $id: ID!\n  $name: String!\n  $enabled: Boolean!\n  $nitcEnabled: Int\n  $nitcCompleteOnExport: Boolean\n) {\n  updateLocation(id: $id, name: $name, enabled: $enabled, nitcEnabled: $nitcEnabled, nitcCompleteOnExport: $nitcCompleteOnExport) {\n    id\n    name\n    enabled\n    nitcEnabled\n    nitcCompleteOnExport\n  }\n}\n"
  }
};
})();

(node as any).hash = "e853a0d2fce53648f02872836615bee8";

export default node;
