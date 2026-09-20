/**
 * @generated SignedSource<<2fb7f0833cc33eac51ab82e8e4f9f50a>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type LocationEditMutation$variables = {
  enabled: boolean;
  gamificationEnabled?: boolean | null | undefined;
  id: string;
  name: string;
  nitcCompleteOnExport?: boolean | null | undefined;
  nitcEnabled?: number | null | undefined;
};
export type LocationEditMutation$data = {
  readonly updateLocation: {
    readonly enabled: boolean;
    readonly gamificationEnabled: boolean;
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
  "name": "gamificationEnabled"
},
v2 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "id"
},
v3 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "name"
},
v4 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "nitcCompleteOnExport"
},
v5 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "nitcEnabled"
},
v6 = [
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
        "name": "gamificationEnabled",
        "variableName": "gamificationEnabled"
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
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "gamificationEnabled",
        "storageKey": null
      }
    ],
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": [
      (v0/*: any*/),
      (v1/*: any*/),
      (v2/*: any*/),
      (v3/*: any*/),
      (v4/*: any*/),
      (v5/*: any*/)
    ],
    "kind": "Fragment",
    "metadata": null,
    "name": "LocationEditMutation",
    "selections": (v6/*: any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [
      (v2/*: any*/),
      (v3/*: any*/),
      (v0/*: any*/),
      (v5/*: any*/),
      (v4/*: any*/),
      (v1/*: any*/)
    ],
    "kind": "Operation",
    "name": "LocationEditMutation",
    "selections": (v6/*: any*/)
  },
  "params": {
    "cacheID": "00aae34dbd77a8cb2a8a0f75a205bb1b",
    "id": null,
    "metadata": {},
    "name": "LocationEditMutation",
    "operationKind": "mutation",
    "text": "mutation LocationEditMutation(\n  $id: ID!\n  $name: String!\n  $enabled: Boolean!\n  $nitcEnabled: Int\n  $nitcCompleteOnExport: Boolean\n  $gamificationEnabled: Boolean\n) {\n  updateLocation(id: $id, name: $name, enabled: $enabled, nitcEnabled: $nitcEnabled, nitcCompleteOnExport: $nitcCompleteOnExport, gamificationEnabled: $gamificationEnabled) {\n    id\n    name\n    enabled\n    nitcEnabled\n    nitcCompleteOnExport\n    gamificationEnabled\n  }\n}\n"
  }
};
})();

(node as any).hash = "3a0869e1cb3436054be50e96efb03300";

export default node;
